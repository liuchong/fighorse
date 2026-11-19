//! High-level Figma team/folder resource catalog (OpenAPI 0.47.0 v2 Folders).

use crate::api::{
    components as components_api, files as files_api, folders as folders_api, styles as styles_api,
    users as users_api,
};
use crate::error::{Error, Result};
use crate::url as figma_url;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};

const LIBRARY_PAGE_SIZE: &str = "1000";
const MAX_LIBRARY_PAGES: usize = 100;
const DEFAULT_MAX_FOLDERS: usize = 200;

#[derive(Debug, Clone)]
pub struct CatalogOpts<'a> {
    pub figma_url: Option<&'a str>,
    pub team_id: Option<&'a str>,
    pub folder_id: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub include_libraries: bool,
    pub branch_data: bool,
    pub probe_file_access: bool,
    pub max_probes: usize,
    pub max_folders: usize,
}

impl Default for CatalogOpts<'_> {
    fn default() -> Self {
        Self {
            figma_url: None,
            team_id: None,
            folder_id: None,
            project_id: None,
            include_libraries: true,
            branch_data: true,
            probe_file_access: false,
            max_probes: 25,
            max_folders: DEFAULT_MAX_FOLDERS,
        }
    }
}

#[derive(Debug)]
pub struct CatalogOutcome {
    pub report: Value,
    pub blocked: bool,
}

#[derive(Debug, Clone)]
enum SourceKind {
    Team(String),
    Folder(String),
    BrowserRoot,
}

#[derive(Debug, Clone)]
struct Source {
    kind: SourceKind,
    input_kind: &'static str,
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resolve_source(opts: &CatalogOpts<'_>) -> Result<Source> {
    let team_id = non_empty(opts.team_id);
    let folder_id = non_empty(opts.folder_id);
    let project_id = non_empty(opts.project_id);

    if folder_id.is_some() && project_id.is_some() && folder_id != project_id {
        return Err(Error::Usage(
            "Provide either folder_id or project_id, not conflicting IDs".to_string(),
        ));
    }
    let explicit_folder = folder_id.or(project_id);
    if team_id.is_some() && explicit_folder.is_some() {
        return Err(Error::Usage(
            "Use exactly one of team_id or folder_id/project_id".to_string(),
        ));
    }
    if let Some(team_id) = team_id {
        return Ok(Source {
            kind: SourceKind::Team(team_id.to_string()),
            input_kind: "team_id",
        });
    }
    if let Some(fid) = explicit_folder {
        return Ok(Source {
            kind: SourceKind::Folder(fid.to_string()),
            input_kind: if folder_id.is_some() {
                "folder_id"
            } else {
                "project_id"
            },
        });
    }

    let input = non_empty(opts.figma_url).ok_or_else(|| {
        Error::Usage("figma_url, team_id, folder_id, or project_id required".to_string())
    })?;
    let parsed = figma_url::parse_figma_url(input);
    if let Some(folder_id) = parsed.folder_id.clone() {
        let input_kind = if parsed.url_role() == "folder_browser" {
            "folder_url"
        } else {
            "project_url"
        };
        return Ok(Source {
            kind: SourceKind::Folder(folder_id),
            input_kind,
        });
    }
    if let Some(project_id) = parsed.project_id {
        return Ok(Source {
            kind: SourceKind::Folder(project_id),
            input_kind: "project_url",
        });
    }
    if let Some(team_id) = parsed.team_id {
        return Ok(Source {
            kind: SourceKind::Team(team_id),
            input_kind: "team_url",
        });
    }
    if parsed.browser_root_id.is_some() && parsed.kind.as_deref() == Some("files") {
        return Ok(Source {
            kind: SourceKind::BrowserRoot,
            input_kind: "browser_root_url",
        });
    }
    Err(Error::Usage(
        "The catalog source must be a Figma team/folder/project browser URL or an explicit team_id/folder_id/project_id"
            .to_string(),
    ))
}

fn source_json(source: &Source) -> Value {
    match &source.kind {
        SourceKind::Team(team_id) => json!({
            "input_kind": source.input_kind,
            "team_id": team_id,
            "folder_id": null,
            "project_id": null,
        }),
        SourceKind::Folder(folder_id) => json!({
            "input_kind": source.input_kind,
            "team_id": null,
            "folder_id": folder_id,
            "project_id": folder_id,
        }),
        SourceKind::BrowserRoot => json!({
            "input_kind": source.input_kind,
            "team_id": null,
            "folder_id": null,
            "project_id": null,
        }),
    }
}

fn request_json(opts: &CatalogOpts<'_>) -> Value {
    json!({
        "include_libraries": opts.include_libraries,
        "branch_data": opts.branch_data,
        "probe_file_access": opts.probe_file_access,
        "max_probes": opts.max_probes,
        "max_folders": opts.max_folders,
    })
}

fn safe_diagnostic(code: &str, scope: &str, error: &Error, next_step: &str) -> Value {
    json!({
        "code": code,
        "scope": scope,
        "http_status": error.figma_status(),
        "message": error.figma_message(),
        "next_step": next_step,
    })
}

fn local_diagnostic(code: &str, scope: &str, message: &str, next_step: &str) -> Value {
    json!({
        "code": code,
        "scope": scope,
        "http_status": null,
        "message": message,
        "next_step": next_step,
    })
}

fn base_report(source: &Source, opts: &CatalogOpts<'_>) -> Value {
    json!({
        "kind": "fighorse.resource-catalog.v1",
        "status": "blocked",
        "source": source_json(source),
        "request": request_json(opts),
        "auth_probe": {"ok": null},
        "summary": {
            "folders": 0,
            "projects": 0,
            "files": 0,
            "branches": 0,
            "components": 0,
            "component_sets": 0,
            "styles": 0,
            "probed_files": 0,
            "readable_files": 0,
            "failed_probes": 0,
        },
        "folders": [],
        "projects": [],
        "team_library": {
            "components": {"status": "skipped", "items": []},
            "component_sets": {"status": "skipped", "items": []},
            "styles": {"status": "skipped", "items": []},
        },
        "diagnostics": [],
        "next_tools": [
            {
                "tool": "get_design_package",
                "for_each": "folders[].files[].key",
                "reason": "Build implementation context for a selected file or node."
            },
            {
                "tool": "search_design_system",
                "for_each": "source.team_id or folders[].files[].key",
                "reason": "Search published components, component sets, styles, and variables."
            },
            {
                "tool": "get_file",
                "for_each": "folders[].files[].key",
                "reason": "Read the full document tree only for files that need inspection."
            },
            {
                "tool": "get_local_variables",
                "for_each": "folders[].files[].key",
                "reason": "Read variables explicitly when the Figma plan and token permit it."
            },
            {
                "tool": "get_dev_resources",
                "for_each": "folders[].files[].key",
                "reason": "Read file dev-resource links explicitly."
            },
            {
                "tool": "get_image_fills",
                "for_each": "folders[].files[].key",
                "reason": "Discover original image fills without downloading them automatically."
            }
        ],
    })
}

fn blocked_outcome(
    source: &Source,
    opts: &CatalogOpts<'_>,
    auth_probe: Value,
    diagnostics: Vec<Value>,
) -> CatalogOutcome {
    let mut report = base_report(source, opts);
    report["auth_probe"] = auth_probe;
    report["diagnostics"] = Value::Array(diagnostics);
    CatalogOutcome {
        report,
        blocked: true,
    }
}

fn error_code(
    error: &Error,
    forbidden: &'static str,
    rate_limited: &'static str,
    failed: &'static str,
) -> &'static str {
    match error.figma_status() {
        Some(403) => forbidden,
        Some(429) => rate_limited,
        _ => failed,
    }
}

fn normalize_file(file: &Value) -> Value {
    json!({
        "key": file.get("key").cloned().unwrap_or(Value::Null),
        "name": file.get("name").cloned().unwrap_or(Value::Null),
        "thumbnail_url": file.get("thumbnail_url").cloned().unwrap_or(Value::Null),
        "last_modified": file.get("last_modified").cloned().unwrap_or(Value::Null),
        "branches": file.get("branches").cloned().unwrap_or_else(|| json!([])),
        "access": null,
    })
}

fn normalize_folder(folder: &Value, files: Vec<Value>, status: &str) -> Value {
    json!({
        "id": folder.get("id").cloned().unwrap_or(Value::Null),
        "name": folder.get("name").cloned().unwrap_or(Value::Null),
        "parent_folder_id": folder.get("parent_folder_id").cloned().unwrap_or(Value::Null),
        "status": status,
        "files": files,
    })
}

enum LibraryKind {
    Components,
    ComponentSets,
    Styles,
}

impl LibraryKind {
    fn item_key(&self) -> &'static str {
        match self {
            LibraryKind::Components => "components",
            LibraryKind::ComponentSets => "component_sets",
            LibraryKind::Styles => "styles",
        }
    }
}

async fn fetch_library_page(
    token: &str,
    team_id: &str,
    kind: &LibraryKind,
    after: Option<&str>,
) -> Result<Value> {
    match kind {
        LibraryKind::Components => {
            components_api::get_team_components(
                token,
                team_id,
                Some(LIBRARY_PAGE_SIZE),
                after,
                None,
            )
            .await
        }
        LibraryKind::ComponentSets => {
            components_api::get_team_component_sets(
                token,
                team_id,
                Some(LIBRARY_PAGE_SIZE),
                after,
                None,
            )
            .await
        }
        LibraryKind::Styles => {
            styles_api::get_team_styles(token, team_id, Some(LIBRARY_PAGE_SIZE), after, None).await
        }
    }
}

fn cursor_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    }
}

async fn collect_library(
    token: &str,
    team_id: &str,
    kind: LibraryKind,
) -> (Vec<Value>, Option<Error>) {
    let mut items = Vec::new();
    let mut after: Option<String> = None;
    let mut seen_cursors = HashSet::new();
    let mut seen_item_keys = HashSet::new();
    for _ in 0..MAX_LIBRARY_PAGES {
        let page = match fetch_library_page(token, team_id, &kind, after.as_deref()).await {
            Ok(page) => page,
            Err(error) => return (items, Some(error)),
        };
        if let Some(page_items) = page["meta"][kind.item_key()].as_array() {
            for item in page_items {
                match item["key"].as_str() {
                    Some(key) if seen_item_keys.insert(key.to_string()) => items.push(item.clone()),
                    Some(_) => {}
                    None => items.push(item.clone()),
                }
            }
        }
        let next = cursor_string(page["meta"]["cursor"].get("after"));
        let Some(next) = next else {
            return (items, None);
        };
        if !seen_cursors.insert(next.clone()) {
            return (
                items,
                Some(Error::Other(
                    "Figma returned a repeated pagination cursor".to_string(),
                )),
            );
        }
        after = Some(next);
    }
    (
        items,
        Some(Error::Other(format!(
            "Figma library pagination exceeded {MAX_LIBRARY_PAGES} pages"
        ))),
    )
}

fn library_failure_diagnostic(kind: &str, error: &Error) -> Value {
    safe_diagnostic(
        error_code(
            error,
            "team_library_forbidden",
            "team_library_rate_limited",
            "team_library_failed",
        ),
        kind,
        error,
        "Check `team_library_content:read`, team access, and Figma rate limits; the folder/file catalog remains usable.",
    )
}

fn count_branches(folders: &[Value]) -> usize {
    folders
        .iter()
        .filter_map(|folder| folder["files"].as_array())
        .flatten()
        .filter_map(|file| file["branches"].as_array())
        .map(Vec::len)
        .sum()
}

async fn probe_files(
    token: &str,
    folders: &mut [Value],
    max_probes: usize,
    diagnostics: &mut Vec<Value>,
) -> (usize, usize, usize, bool) {
    let mut attempted = 0;
    let mut readable = 0;
    let mut failed = 0;
    let mut truncated = false;
    for folder in folders {
        let Some(files) = folder["files"].as_array_mut() else {
            continue;
        };
        for file in files {
            if max_probes != 0 && attempted >= max_probes {
                truncated = true;
                continue;
            }
            let Some(file_key) = file["key"].as_str().map(String::from) else {
                continue;
            };
            attempted += 1;
            match files_api::get_file(
                token,
                &file_key,
                files_api::GetFileParams {
                    depth: Some("1"),
                    ..Default::default()
                },
            )
            .await
            {
                Ok(data) => {
                    readable += 1;
                    let page_count = data["document"]["children"]
                        .as_array()
                        .map(Vec::len)
                        .unwrap_or(0);
                    file["access"] = json!({"status": "readable", "page_count": page_count});
                }
                Err(error) => {
                    failed += 1;
                    file["access"] = json!({
                        "status": "failed",
                        "http_status": error.figma_status(),
                        "message": error.figma_message(),
                    });
                    diagnostics.push(safe_diagnostic(
                        error_code(
                            &error,
                            "file_content_forbidden",
                            "file_content_rate_limited",
                            "file_content_probe_failed",
                        ),
                        "file_content",
                        &error,
                        "Check `file_content:read` and file access, then retry only the affected file.",
                    ));
                    if error.figma_status() == Some(429) {
                        return (attempted, readable, failed, true);
                    }
                }
            }
        }
    }
    if truncated {
        diagnostics.push(local_diagnostic(
            "file_probe_limit_reached",
            "file_content",
            "Not every file was probed because max_probes was reached.",
            "Increase max_probes or set it to 0 only when an unbounded read-only probe is intended.",
        ));
    }
    (attempted, readable, failed, truncated)
}

pub fn local_catalog_outcome(opts: &CatalogOpts<'_>) -> Result<Option<CatalogOutcome>> {
    let source = resolve_source(opts)?;
    if matches!(source.kind, SourceKind::BrowserRoot) {
        return Ok(Some(blocked_outcome(
            &source,
            opts,
            json!({"ok": null}),
            vec![local_diagnostic(
                "browser_root_not_enumerable",
                "source",
                "The public Figma REST API cannot discover team IDs from a files browser root.",
                "Open a team or folder page and provide a URL containing `/team/<team-id>` or `/folder/<folder-id>`.",
            )],
        )));
    }
    Ok(None)
}

fn folder_id_str(folder: &Value) -> Option<String> {
    match folder.get("id") {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

pub async fn get_resource_catalog(token: &str, opts: CatalogOpts<'_>) -> Result<CatalogOutcome> {
    if let Some(outcome) = local_catalog_outcome(&opts)? {
        return Ok(outcome);
    }
    let source = resolve_source(&opts)?;

    // Plan Access Tokens and narrowly-scoped OAuth tokens may lack `current_user:read`
    // (returning HTTP 403 on `/v1/me`) while still holding `folders:read`.
    let auth_probe = match users_api::get_me(token).await {
        Ok(_) => json!({"ok": true}),
        Err(error) if error.figma_status() == Some(403) => json!({
            "ok": false,
            "http_status": 403,
            "reason": "unscoped_or_plan_token",
            "message": error.figma_message(),
        }),
        Err(error) => {
            return Ok(blocked_outcome(
                &source,
                &opts,
                json!({"ok": false}),
                vec![safe_diagnostic(
                    error_code(
                        &error,
                        "identity_forbidden",
                        "identity_rate_limited",
                        "identity_probe_failed",
                    ),
                    "current_user",
                    &error,
                    "Create a valid Figma token (or set FIGHORSE_AUTH_SCHEME=bearer for OAuth tokens), store it locally, and retry.",
                )],
            ));
        }
    };

    let mut report = base_report(&source, &opts);
    report["auth_probe"] = auth_probe.clone();
    let mut diagnostics = Vec::new();
    let mut partial = false;
    let mut folders = Vec::new();

    let initial_folders: Vec<Value> = match &source.kind {
        SourceKind::Team(team_id) => match folders_api::get_team_folders(token, team_id).await {
            Ok(data) => data["folders"]
                .as_array()
                .or_else(|| data["projects"].as_array())
                .cloned()
                .unwrap_or_default(),
            Err(error) => {
                return Ok(blocked_outcome(
                    &source,
                    &opts,
                    auth_probe,
                    vec![safe_diagnostic(
                        error_code(
                            &error,
                            "folders_access_forbidden",
                            "folders_rate_limited",
                            "folders_list_failed",
                        ),
                        "folders",
                        &error,
                        "Check `folders:read` (or legacy `projects:read`) and verify that the token can access this team.",
                    )],
                ));
            }
        },
        SourceKind::Folder(folder_id) => {
            vec![json!({"id": folder_id, "name": null, "parent_folder_id": null})]
        }
        SourceKind::BrowserRoot => unreachable!(),
    };

    let mut queue: VecDeque<Value> = VecDeque::new();
    let mut visited_ids: HashSet<String> = HashSet::new();
    for item in initial_folders {
        if let Some(id) = folder_id_str(&item) {
            if visited_ids.insert(id) {
                queue.push_back(item);
            }
        } else {
            queue.push_back(item);
        }
    }

    let initial_count = queue.len();
    let mut successful_folder_lists = 0usize;
    let mut stop_folder_requests = false;
    let mut folder_limit_reached = false;
    let max_folders = opts.max_folders;

    while let Some(folder) = queue.pop_front() {
        if stop_folder_requests {
            folders.push(normalize_folder(&folder, Vec::new(), "not_attempted"));
            continue;
        }
        if max_folders != 0 && folders.len() >= max_folders {
            folder_limit_reached = true;
            partial = true;
            folders.push(normalize_folder(&folder, Vec::new(), "not_attempted"));
            continue;
        }
        let Some(folder_id) = folder_id_str(&folder) else {
            partial = true;
            diagnostics.push(local_diagnostic(
                "folder_id_missing",
                "folders",
                "Figma returned a folder without an id.",
                "Retry the request; report the response-shape change if it persists.",
            ));
            continue;
        };

        let branch_data = if opts.branch_data {
            Some("true")
        } else {
            Some("false")
        };

        // Enumerate nested child folders (OpenAPI 0.47.0 v2 Folders).
        match folders_api::get_folder_folders(token, &folder_id).await {
            Ok(sub_data) => {
                if let Some(children) = sub_data["folders"].as_array() {
                    for child in children {
                        let mut enriched = child.clone();
                        if enriched.get("parent_folder_id").is_none()
                            || enriched["parent_folder_id"].is_null()
                        {
                            enriched["parent_folder_id"] = Value::String(folder_id.clone());
                        }
                        if let Some(cid) = folder_id_str(&enriched)
                            && visited_ids.insert(cid)
                        {
                            queue.push_back(enriched);
                        }
                    }
                }
            }
            Err(error) => {
                // Treat 404 on subfolder listing as leaf/empty when files still succeed,
                // while surfacing 403/429/5xx diagnostics.
                if error.figma_status() != Some(404) {
                    partial = true;
                    diagnostics.push(safe_diagnostic(
                        error_code(
                            &error,
                            "folder_subfolders_forbidden",
                            "folder_subfolders_rate_limited",
                            "folder_subfolders_failed",
                        ),
                        "folder_subfolders",
                        &error,
                        "Check `folders:read` (or legacy `projects:read`) and access to nested folders.",
                    ));
                    if error.figma_status() == Some(429) {
                        stop_folder_requests = true;
                        folders.push(normalize_folder(&folder, Vec::new(), "failed"));
                        continue;
                    }
                }
            }
        }

        match folders_api::get_folder_files(token, &folder_id, branch_data).await {
            Ok(data) => {
                successful_folder_lists += 1;
                let files = data["files"]
                    .as_array()
                    .map(|files| files.iter().map(normalize_file).collect())
                    .unwrap_or_default();
                let mut normalized = normalize_folder(&folder, files, "ready");
                if normalized["name"].is_null() {
                    normalized["name"] = data.get("name").cloned().unwrap_or(Value::Null);
                }
                folders.push(normalized);
            }
            Err(error) => {
                partial = true;
                diagnostics.push(safe_diagnostic(
                    error_code(
                        &error,
                        "folder_files_forbidden",
                        "folder_files_rate_limited",
                        "folder_files_failed",
                    ),
                    "folder_files",
                    &error,
                    "Check `folders:read` (or legacy `projects:read`) and access to this folder; other folder results were preserved.",
                ));
                folders.push(normalize_folder(&folder, Vec::new(), "failed"));
                if error.figma_status() == Some(429) {
                    stop_folder_requests = true;
                }
            }
        }
    }

    if folder_limit_reached {
        diagnostics.push(local_diagnostic(
            "folder_limit_reached",
            "folders",
            "Nested folder traversal stopped at max_folders.",
            "Increase max_folders or set it to 0 only when unbounded folder traversal is intended.",
        ));
    }

    if initial_count > 0 && successful_folder_lists == 0 {
        report["status"] = Value::String("blocked".to_string());
        report["summary"]["folders"] = json!(folders.len());
        report["summary"]["projects"] = json!(folders.len());
        report["folders"] = Value::Array(folders.clone());
        report["projects"] = Value::Array(folders);
        report["diagnostics"] = Value::Array(diagnostics);
        return Ok(CatalogOutcome {
            report,
            blocked: true,
        });
    }

    let mut component_items = Vec::new();
    let mut component_set_items = Vec::new();
    let mut style_items = Vec::new();
    if opts.include_libraries
        && let SourceKind::Team(team_id) = &source.kind
    {
        for (kind, label) in [
            (LibraryKind::Components, "components"),
            (LibraryKind::ComponentSets, "component_sets"),
            (LibraryKind::Styles, "styles"),
        ] {
            let (items, error) = collect_library(token, team_id, kind).await;
            let status = if error.is_some() { "partial" } else { "ready" };
            match label {
                "components" => component_items = items.clone(),
                "component_sets" => component_set_items = items.clone(),
                "styles" => style_items = items.clone(),
                _ => unreachable!(),
            }
            report["team_library"][label] = json!({"status": status, "items": items});
            if let Some(error) = error {
                partial = true;
                diagnostics.push(library_failure_diagnostic(label, &error));
            }
        }
    }

    let file_count = folders
        .iter()
        .filter_map(|folder| folder["files"].as_array())
        .map(Vec::len)
        .sum::<usize>();
    let branch_count = count_branches(&folders);
    let (probed, readable, failed_probes, probe_truncated) = if opts.probe_file_access {
        probe_files(token, &mut folders, opts.max_probes, &mut diagnostics).await
    } else {
        (0, 0, 0, false)
    };
    partial |= probe_truncated || failed_probes > 0;

    report["status"] = Value::String(if partial { "partial" } else { "ready" }.to_string());
    report["summary"] = json!({
        "folders": folders.len(),
        "projects": folders.len(),
        "files": file_count,
        "branches": branch_count,
        "components": component_items.len(),
        "component_sets": component_set_items.len(),
        "styles": style_items.len(),
        "probed_files": probed,
        "readable_files": readable,
        "failed_probes": failed_probes,
    });
    report["folders"] = Value::Array(folders.clone());
    report["projects"] = Value::Array(folders);
    report["diagnostics"] = Value::Array(diagnostics);
    Ok(CatalogOutcome {
        report,
        blocked: false,
    })
}
