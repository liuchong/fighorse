//! Design system search across published team libraries and file libraries/variables.

use crate::api::{components as components_api, styles as styles_api, variables as variables_api};
use crate::error::{Error, Result};
use crate::url as figma_url;
use serde_json::{Value, json};
use std::collections::HashSet;

const PAGE_SIZE: &str = "1000";
const MAX_PAGES: usize = 25;

#[derive(Debug, Clone)]
pub struct DesignSystemSearchOpts<'a> {
    pub figma_url: Option<&'a str>,
    pub team_id: Option<&'a str>,
    pub file_key: Option<&'a str>,
    pub queries: Vec<String>,
    pub kinds: Vec<String>,
    pub limit: usize,
}

impl Default for DesignSystemSearchOpts<'_> {
    fn default() -> Self {
        Self {
            figma_url: None,
            team_id: None,
            file_key: None,
            queries: Vec::new(),
            kinds: Vec::new(),
            limit: 50,
        }
    }
}

fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|v| !v.is_empty()).map(String::from)
}

fn normalize_kind(raw: &str) -> Result<&'static str> {
    match raw.trim().to_lowercase().as_str() {
        "component" | "components" => Ok("component"),
        "component_set" | "component_sets" | "component-set" | "component-sets" => {
            Ok("component_set")
        }
        "style" | "styles" => Ok("style"),
        "variable" | "variables" => Ok("variable"),
        other => Err(Error::Usage(format!(
            "Unsupported design system asset kind `{other}`. Expected component, component_set, style, or variable."
        ))),
    }
}

fn cursor_after(page: &Value) -> Option<String> {
    match page["meta"]["cursor"].get("after") {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn score_match(queries: &[String], haystacks: &[Option<&str>]) -> Option<u32> {
    if queries.is_empty() {
        return Some(1);
    }
    let lowered: Vec<String> = haystacks
        .iter()
        .flatten()
        .map(|s| s.to_lowercase())
        .collect();
    let mut best: Option<u32> = None;
    for q in queries {
        let mut q_score: Option<u32> = None;
        for (idx, hay) in lowered.iter().enumerate() {
            let weight = if idx == 0 { 10 } else { 3 };
            let candidate = if hay == q {
                Some(100 + weight)
            } else if hay.starts_with(q) {
                Some(60 + weight)
            } else if hay.contains(q) {
                Some(20 + weight)
            } else {
                None
            };
            if let Some(c) = candidate {
                q_score = Some(q_score.map_or(c, |prev| prev.max(c)));
            }
        }
        if let Some(s) = q_score {
            best = Some(best.map_or(s, |prev| prev + s));
        }
    }
    best
}

fn diagnostic(code: &str, scope: &str, err: &Error) -> Value {
    json!({
        "code": code,
        "scope": scope,
        "http_status": err.figma_status(),
        "message": err.figma_message(),
    })
}

pub async fn search_design_system(token: &str, opts: DesignSystemSearchOpts<'_>) -> Result<Value> {
    let parsed = opts
        .figma_url
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(figma_url::parse_figma_url);

    let team_id = non_empty(opts.team_id).or_else(|| parsed.as_ref().and_then(|p| p.team_id.clone()));
    let file_key =
        non_empty(opts.file_key).or_else(|| parsed.as_ref().and_then(|p| p.file_key.clone()));

    if team_id.is_none() && file_key.is_none() {
        return Err(Error::Usage(
            "search_design_system requires team_id, file_key, or a Figma team/file URL".into(),
        ));
    }

    let queries: Vec<String> = opts
        .queries
        .iter()
        .map(|q| q.trim().to_lowercase())
        .filter(|q| !q.is_empty())
        .collect();

    let kinds: Vec<&'static str> = if opts.kinds.is_empty() {
        if file_key.is_some() {
            vec!["component", "component_set", "style", "variable"]
        } else {
            vec!["component", "component_set", "style"]
        }
    } else {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for k in &opts.kinds {
            let normalized = normalize_kind(k)?;
            if seen.insert(normalized) {
                out.push(normalized);
            }
        }
        out
    };

    let limit = if opts.limit == 0 {
        50
    } else {
        opts.limit.min(500)
    };

    let mut scored_matches: Vec<(u32, String, Value)> = Vec::new();
    let mut seen_dedup: HashSet<String> = HashSet::new();
    let mut diagnostics: Vec<Value> = Vec::new();
    let mut sources_succeeded = 0usize;
    let mut sources_attempted = 0usize;

    let mut push_library_item = |asset_kind: &str, item: &Value| {
        let name = item.get("name").and_then(Value::as_str);
        let desc = item.get("description").and_then(Value::as_str);
        let key = item.get("key").and_then(Value::as_str);
        let frame_name = item
            .get("containing_frame")
            .and_then(|f| f.get("name"))
            .and_then(Value::as_str);
        let style_type = item.get("style_type").and_then(Value::as_str);
        let Some(score) = score_match(&queries, &[name, frame_name, desc, style_type, key]) else {
            return;
        };
        let dedup_id = format!(
            "{asset_kind}:{}",
            key.or_else(|| item.get("node_id").and_then(Value::as_str))
                .or(name)
                .unwrap_or("")
        );
        if !seen_dedup.insert(dedup_id) {
            return;
        }
        let sort_name = name.unwrap_or("").to_string();
        scored_matches.push((
            score,
            sort_name,
            json!({
                "asset_kind": asset_kind,
                "key": item.get("key").cloned().unwrap_or(Value::Null),
                "name": item.get("name").cloned().unwrap_or(Value::Null),
                "description": item.get("description").cloned().unwrap_or(Value::Null),
                "file_key": item.get("file_key").cloned().unwrap_or(Value::Null),
                "node_id": item.get("node_id").cloned().unwrap_or(Value::Null),
                "containing_frame": item.get("containing_frame").cloned().unwrap_or(Value::Null),
                "style_type": item.get("style_type").cloned().unwrap_or(Value::Null),
            }),
        ));
    };

    if let Some(tid) = team_id.as_deref() {
        if kinds.contains(&"component") {
            sources_attempted += 1;
            let mut after: Option<String> = None;
            let mut seen_cursors = HashSet::new();
            for _ in 0..MAX_PAGES {
                match components_api::get_team_components(
                    token,
                    tid,
                    Some(PAGE_SIZE),
                    after.as_deref(),
                    None,
                )
                .await
                {
                    Ok(page) => {
                        sources_succeeded += 1;
                        if let Some(items) = page["meta"]["components"].as_array() {
                            for item in items {
                                push_library_item("component", item);
                            }
                        }
                        match cursor_after(&page) {
                            Some(next) if seen_cursors.insert(next.clone()) => after = Some(next),
                            _ => break,
                        }
                    }
                    Err(err) => {
                        diagnostics.push(diagnostic("team_components_failed", "component", &err));
                        break;
                    }
                }
            }
        }
        if kinds.contains(&"component_set") {
            sources_attempted += 1;
            let mut after: Option<String> = None;
            let mut seen_cursors = HashSet::new();
            for _ in 0..MAX_PAGES {
                match components_api::get_team_component_sets(
                    token,
                    tid,
                    Some(PAGE_SIZE),
                    after.as_deref(),
                    None,
                )
                .await
                {
                    Ok(page) => {
                        sources_succeeded += 1;
                        if let Some(items) = page["meta"]["component_sets"].as_array() {
                            for item in items {
                                push_library_item("component_set", item);
                            }
                        }
                        match cursor_after(&page) {
                            Some(next) if seen_cursors.insert(next.clone()) => after = Some(next),
                            _ => break,
                        }
                    }
                    Err(err) => {
                        diagnostics.push(diagnostic(
                            "team_component_sets_failed",
                            "component_set",
                            &err,
                        ));
                        break;
                    }
                }
            }
        }
        if kinds.contains(&"style") {
            sources_attempted += 1;
            let mut after: Option<String> = None;
            let mut seen_cursors = HashSet::new();
            for _ in 0..MAX_PAGES {
                match styles_api::get_team_styles(
                    token,
                    tid,
                    Some(PAGE_SIZE),
                    after.as_deref(),
                    None,
                )
                .await
                {
                    Ok(page) => {
                        sources_succeeded += 1;
                        if let Some(items) = page["meta"]["styles"].as_array() {
                            for item in items {
                                push_library_item("style", item);
                            }
                        }
                        match cursor_after(&page) {
                            Some(next) if seen_cursors.insert(next.clone()) => after = Some(next),
                            _ => break,
                        }
                    }
                    Err(err) => {
                        diagnostics.push(diagnostic("team_styles_failed", "style", &err));
                        break;
                    }
                }
            }
        }
        if kinds.contains(&"variable") && file_key.is_none() {
            diagnostics.push(json!({
                "code": "variables_require_file_key",
                "scope": "variable",
                "http_status": null,
                "message": "Figma REST variables endpoints are scoped to a file_key; pass file_key to search variables.",
            }));
        }
    }

    if let Some(fk) = file_key.as_deref() {
        if kinds.contains(&"component") {
            sources_attempted += 1;
            match components_api::get_file_components(token, fk).await {
                Ok(resp) => {
                    sources_succeeded += 1;
                    if let Some(items) = resp["meta"]["components"].as_array() {
                        for item in items {
                            push_library_item("component", item);
                        }
                    }
                }
                Err(err) => {
                    diagnostics.push(diagnostic("file_components_failed", "component", &err))
                }
            }
        }
        if kinds.contains(&"component_set") {
            sources_attempted += 1;
            match components_api::get_file_component_sets(token, fk).await {
                Ok(resp) => {
                    sources_succeeded += 1;
                    if let Some(items) = resp["meta"]["component_sets"].as_array() {
                        for item in items {
                            push_library_item("component_set", item);
                        }
                    }
                }
                Err(err) => diagnostics.push(diagnostic(
                    "file_component_sets_failed",
                    "component_set",
                    &err,
                )),
            }
        }
        if kinds.contains(&"style") {
            sources_attempted += 1;
            match styles_api::get_file_styles(token, fk).await {
                Ok(resp) => {
                    sources_succeeded += 1;
                    if let Some(items) = resp["meta"]["styles"].as_array() {
                        for item in items {
                            push_library_item("style", item);
                        }
                    }
                }
                Err(err) => diagnostics.push(diagnostic("file_styles_failed", "style", &err)),
            }
        }
        if kinds.contains(&"variable") {
            sources_attempted += 1;
            match variables_api::get_local_variables(token, fk).await {
                Ok(resp) => {
                    sources_succeeded += 1;
                    let collections = resp["meta"]["variableCollections"]
                        .as_object()
                        .cloned()
                        .unwrap_or_default();
                    if let Some(vars) = resp["meta"]["variables"].as_object() {
                        for (var_id, var) in vars {
                            let name = var.get("name").and_then(Value::as_str);
                            let desc = var.get("description").and_then(Value::as_str);
                            let key = var.get("key").and_then(Value::as_str);
                            let resolved_type = var.get("resolvedType").and_then(Value::as_str);
                            let col_id = var.get("variableCollectionId").and_then(Value::as_str);
                            let col_name = col_id
                                .and_then(|cid| collections.get(cid))
                                .and_then(|c| c.get("name"))
                                .and_then(Value::as_str);
                            let Some(score) = score_match(
                                &queries,
                                &[name, col_name, desc, resolved_type, key, Some(var_id)],
                            ) else {
                                continue;
                            };
                            let dedup_id = format!("variable:{}", key.unwrap_or(var_id));
                            if !seen_dedup.insert(dedup_id) {
                                continue;
                            }
                            scored_matches.push((
                                score,
                                name.unwrap_or("").to_string(),
                                json!({
                                    "asset_kind": "variable",
                                    "id": var_id,
                                    "key": var.get("key").cloned().unwrap_or(Value::Null),
                                    "name": var.get("name").cloned().unwrap_or(Value::Null),
                                    "description": var.get("description").cloned().unwrap_or(Value::Null),
                                    "file_key": fk,
                                    "collection_id": col_id,
                                    "collection_name": col_name,
                                    "resolved_type": var.get("resolvedType").cloned().unwrap_or(Value::Null),
                                    "code_syntax": var.get("codeSyntax").cloned().unwrap_or(Value::Null),
                                }),
                            ));
                        }
                    }
                }
                Err(err) => {
                    diagnostics.push(diagnostic("local_variables_failed", "variable", &err));
                }
            }
        }
    }

    if sources_attempted > 0 && sources_succeeded == 0 {
        return Err(Error::Other(format!(
            "All requested design system sources failed: {}",
            serde_json::to_string(&diagnostics).unwrap_or_default()
        )));
    }

    scored_matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let total_matches = scored_matches.len();
    let truncated = total_matches > limit;
    let matches: Vec<Value> = scored_matches
        .into_iter()
        .take(limit)
        .map(|(_, _, item)| item)
        .collect();

    Ok(json!({
        "kind": "fighorse.design-system-search.v1",
        "status": if diagnostics.is_empty() { "ready" } else { "partial" },
        "source": {
            "team_id": team_id,
            "file_key": file_key,
        },
        "queries": queries,
        "kinds": kinds,
        "total_matches": total_matches,
        "truncated": truncated,
        "matches": matches,
        "diagnostics": diagnostics,
    }))
}
