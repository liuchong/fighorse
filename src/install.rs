//! Installation artifact generation for fighorse runtime, MCP clients,
//! services, and skills.
//!
//! Installation artifact generation. Commands are dry-run/artifact-generating by
//! default; `apply` mutates detected client configs, skill locations, PATH
//! links, and service managers.

use crate::config;
use crate::error::{Error, Result};
use crate::experience::{self, ScopeOpts};
use crate::guidance;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod clients;
pub mod model;
pub mod service;
pub mod skills;
pub mod transaction;

use clients::{ClientKind, ClientSpec};

pub const SUPPORTED_CLIENTS: &[&str] = &[
    "cursor",
    "codex",
    "kimi",
    "kimi-cli",
    "claude",
    "opencode",
    "openclaw",
    "hermes-agent",
    "generic",
];

fn home_os() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn fighorse_home(home: Option<&str>) -> PathBuf {
    home.map(PathBuf::from)
        .unwrap_or_else(config::fighorse_home)
}

fn now_iso() -> String {
    experience::now_iso_public()
}

fn safe_timestamp() -> String {
    now_iso().replace([':', '.'], "-")
}

fn file_exists(p: &Path) -> bool {
    p.exists()
}

fn mkdirp(dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    Ok(dir.to_path_buf())
}

fn write_text(file: &Path, content: &str) -> Result<PathBuf> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file, content)?;
    Ok(file.to_path_buf())
}

fn write_json(file: &Path, data: &Value) -> Result<PathBuf> {
    write_text(file, &serde_json::to_string_pretty(data)?)
}

fn read_json_object(file: &Path) -> Map<String, Value> {
    match std::fs::read_to_string(file) {
        Ok(c) => match serde_json::from_str::<Value>(&c) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        },
        Err(_) => Map::new(),
    }
}

fn backup_file(file: &Path) -> Option<PathBuf> {
    if file_exists(file) {
        let backup = PathBuf::from(format!("{}.bak.{}", file.display(), safe_timestamp()));
        std::fs::copy(file, &backup).ok()?;
        Some(backup)
    } else {
        None
    }
}

fn write_text_with_backup(file: &Path, content: &str) -> Result<PathBuf> {
    let current = std::fs::read_to_string(file).ok();
    if current.as_deref() != Some(content) {
        backup_file(file);
        write_text(file, content)?;
    }
    Ok(file.to_path_buf())
}

fn write_json_with_backup(file: &Path, data: &Value) -> Result<PathBuf> {
    write_text_with_backup(file, &serde_json::to_string_pretty(data)?)
}

fn normalize_client(client: &str) -> String {
    let c = client.to_lowercase();
    if c == "kimi-cli" {
        return "kimi".to_string();
    }
    if SUPPORTED_CLIENTS.contains(&c.as_str()) {
        c
    } else {
        "generic".to_string()
    }
}

fn split_list(s: &str) -> Vec<String> {
    if s.trim().is_empty() {
        return vec![];
    }
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Resolve the client list from `--client` / `--clients`.
pub fn coerce_clients(client: Option<&str>, clients: Option<&str>) -> Vec<String> {
    let items: Vec<String> = match clients {
        Some(c) if !c.trim().is_empty() => split_list(c),
        _ => match client {
            Some(c) => vec![c.to_string()],
            None => vec!["generic".to_string()],
        },
    };
    if items.iter().any(|i| i.trim().to_lowercase() == "none") {
        return vec![];
    }
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .map(|c| normalize_client(c))
        .filter(|c| seen.insert(c.clone()))
        .collect()
}

fn path_dirs() -> Vec<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    std::env::var("PATH")
        .unwrap_or_default()
        .split(sep)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn executable_candidates(command: &str) -> Vec<String> {
    let exts: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    if command.trim().is_empty() {
        return vec![];
    }
    if PathBuf::from(command).is_absolute() || command.contains('/') || command.contains('\\') {
        return exts.iter().map(|e| format!("{command}{e}")).collect();
    }
    let mut out = Vec::new();
    for dir in path_dirs() {
        for ext in exts {
            out.push(
                PathBuf::from(&dir)
                    .join(format!("{command}{ext}"))
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    out
}

/// Find an executable on PATH.
pub fn executable_path(command: &str) -> Option<String> {
    executable_candidates(command)
        .into_iter()
        .find(|c| PathBuf::from(c).exists())
}

fn expand_home_path(p: &str) -> String {
    if p.trim().is_empty() {
        p.to_string()
    } else if p == "~" {
        home_os().to_string_lossy().into_owned()
    } else if let Some(rest) = p.strip_prefix("~/") {
        home_os().join(rest).to_string_lossy().into_owned()
    } else {
        p.to_string()
    }
}

fn absolute_path(p: Option<&str>) -> Option<String> {
    let p = p?;
    if p.trim().is_empty() {
        return None;
    }
    let expanded = expand_home_path(p);
    let pb = PathBuf::from(&expanded);
    if pb.is_absolute() {
        Some(expanded)
    } else {
        Some(
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&expanded)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

fn default_binary_target(home: Option<&str>) -> PathBuf {
    fighorse_home(home).join("bin").join("fighorse")
}

fn path_preferred_link_dirs() -> Vec<String> {
    let home = home_os();
    let candidates = [
        home.join("bin"),
        home.join(".local").join("bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    let dirs = path_dirs();
    let mut seen = std::collections::HashSet::new();
    candidates
        .iter()
        .map(|c| c.to_string_lossy().into_owned())
        .filter(|c| dirs.contains(c))
        .filter(|c| seen.insert(c.clone()))
        .collect()
}

fn current_executable_path() -> String {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "fighorse".to_string())
}

/// Resolve an installed command to an absolute path for service/stdio configs.
fn command_path(command: &str, home: Option<&str>) -> String {
    if command.trim().is_empty() {
        return default_binary_target(home).to_string_lossy().into_owned();
    }
    let expanded = expand_home_path(command);
    if PathBuf::from(&expanded).is_absolute()
        || command.contains('/')
        || command.contains('\\')
        || command.starts_with('~')
    {
        return absolute_path(Some(command)).unwrap_or(expanded);
    }
    executable_path(command)
        .unwrap_or_else(|| default_binary_target(home).to_string_lossy().into_owned())
}

fn install_path_to_target(p: Option<&str>, home: Option<&str>) -> PathBuf {
    let abs = absolute_path(p);
    match abs {
        None => default_binary_target(home),
        Some(p) => {
            let pb = PathBuf::from(&p);
            if !p.ends_with('/')
                && !pb.is_dir()
                && pb.file_name().and_then(|f| f.to_str()) == Some("fighorse")
            {
                pb
            } else {
                pb.join("fighorse")
            }
        }
    }
}

// --- Static instruction content ---

/// The fighorse skill markdown (SKILL.md).
pub fn skill_markdown() -> String {
    include_str!("install/skill.md").to_string()
}

/// The fighorse agent instructions (AGENTS.md).
pub fn agents_markdown() -> String {
    include_str!("install/agents.md").to_string()
}

/// The Cursor rule (.mdc) — front matter plus the agent instructions.
pub fn cursor_rule() -> String {
    format!(
        "---\ndescription: Use fighorse for Figma design replication\nalwaysApply: false\n---\n\n{}",
        agents_markdown()
    )
}

// --- MCP server config shapes ---

fn mcp_stdio_config(command: &str, home: Option<&str>) -> Value {
    json!({
        "command": command,
        "args": ["mcp", "serve", "--transport", "stdio"],
        "env": {
            "FIGHORSE_MCP_MODE": "readonly",
            "FIGHORSE_MCP_LOCAL_WRITE": "deny",
            "FIGHORSE_HOME": fighorse_home(home).to_string_lossy()
        }
    })
}

fn mcp_server_config(
    transport: &str,
    port: i64,
    command: &str,
    home: Option<&str>,
) -> Result<Value> {
    let command = command_path(command, home);
    match transport {
        "http" => Ok(json!({"transport": "http", "url": format!("http://127.0.0.1:{port}/mcp")})),
        "stdio" => Ok(mcp_stdio_config(&command, home)),
        "sse" => Err(Error::Usage(
            "Legacy SSE transport is retired. Use --transport http and the /mcp endpoint.".into(),
        )),
        other => Err(Error::Usage(format!(
            "Unknown client transport: {other}. Expected http or explicit stdio."
        ))),
    }
}

fn codex_toml(server: &Value, command: &str, home: Option<&str>) -> String {
    if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
        format!(
            "[mcp_servers.fighorse]\nurl = \"{url}\"\nenabled = true\nstartup_timeout_sec = 60\n"
        )
    } else {
        format!(
            "[mcp_servers.fighorse]\ncommand = \"{command}\"\nargs = [\"mcp\", \"serve\", \"--transport\", \"stdio\"]\nenabled = true\nstartup_timeout_sec = 60\n\n[mcp_servers.fighorse.env]\nFIGHORSE_MCP_MODE = \"readonly\"\nFIGHORSE_MCP_LOCAL_WRITE = \"deny\"\nFIGHORSE_HOME = \"{}\"\n",
            fighorse_home(home).to_string_lossy()
        )
    }
}

fn write_skill_set(base: &Path) -> Result<Vec<Value>> {
    let skill = base.join("SKILL.md");
    let agents = base.join("AGENTS.md");
    let cursor = base.join("cursor-rule.mdc");
    write_text(&skill, &skill_markdown())?;
    write_text(&agents, &agents_markdown())?;
    write_text(&cursor, &cursor_rule())?;
    Ok(vec![
        Value::String(skill.to_string_lossy().into_owned()),
        Value::String(agents.to_string_lossy().into_owned()),
        Value::String(cursor.to_string_lossy().into_owned()),
    ])
}

fn run_command(command: &str, args: &[&str], env: &[(&str, String)]) -> Value {
    let mut cmd = Command::new(command);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    match cmd.output() {
        Ok(out) => {
            let code = out
                .status
                .code()
                .unwrap_or(if out.status.success() { 0 } else { 1 });
            json!({
                "command": command,
                "args": args,
                "exit_code": code,
                "ok": out.status.success(),
                "stdout": String::from_utf8_lossy(&out.stdout),
                "stderr": String::from_utf8_lossy(&out.stderr),
                "error": Value::Null,
            })
        }
        Err(e) => json!({
            "command": command,
            "args": args,
            "exit_code": 1,
            "ok": false,
            "stdout": "",
            "stderr": "",
            "error": e.to_string(),
        }),
    }
}

// --- Binary install ---

fn copy_executable(source: &str, target: &Path) -> Result<PathBuf> {
    if source.trim().is_empty() {
        return Err(Error::Other(
            "--source is required when applying binary installation".into(),
        ));
    }
    let src = PathBuf::from(source);
    if !src.exists() {
        return Err(Error::Other(format!("Binary source not found: {source}")));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&src, target)?;
    set_executable(target);
    Ok(target.to_path_buf())
}

fn set_executable(target: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(target) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(target, perms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = target;
    }
}

fn symlink_or_copy(target: &Path, link: &Path) -> Result<PathBuf> {
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if link.exists() {
        let _ = std::fs::remove_file(link);
    }
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(target, link).is_err() {
            std::fs::copy(target, link)?;
            set_executable(link);
        }
    }
    #[cfg(not(unix))]
    {
        std::fs::copy(target, link)?;
    }
    Ok(link.to_path_buf())
}

fn requested_binary_links(link_dir: Option<&str>, link_dirs: Option<&str>) -> Vec<PathBuf> {
    let mut requested: Vec<String> = split_list(link_dirs.unwrap_or(""));
    if let Some(link_dir) = link_dir {
        requested.push(link_dir.to_string());
    }
    let disabled = requested
        .iter()
        .any(|directory| directory.trim().eq_ignore_ascii_case("none"));
    let explicit = requested.iter().any(|directory| {
        !directory.trim().is_empty() && !directory.trim().eq_ignore_ascii_case("none")
    });
    let directories = if disabled {
        Vec::new()
    } else if explicit {
        requested
    } else {
        path_preferred_link_dirs()
    };
    let mut seen = std::collections::HashSet::new();
    directories
        .into_iter()
        .filter(|directory| !directory.trim().is_empty())
        .filter_map(|directory| absolute_path(Some(&directory)))
        .filter(|directory| seen.insert(directory.clone()))
        .map(|directory| PathBuf::from(directory).join("fighorse"))
        .collect()
}

/// Install the CLI binary and PATH links.
pub fn install_binary(
    source: Option<&str>,
    target: Option<&str>,
    link_dir: Option<&str>,
    link_dirs: Option<&str>,
    home: Option<&str>,
    apply: bool,
) -> Result<Value> {
    let source = absolute_path(source);
    let target = absolute_path(target)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_binary_target(home));

    let links = requested_binary_links(link_dir, link_dirs);
    let all_dirs: Vec<String> = links
        .iter()
        .filter_map(|link| link.parent())
        .map(|directory| directory.to_string_lossy().into_owned())
        .collect();

    let applied = if apply {
        let src = source.clone().ok_or_else(|| {
            Error::Other("--source is required when applying binary installation".into())
        })?;
        let binary = copy_executable(&src, &target)?;
        let mut applied_links = Vec::new();
        let mut skipped_links = Vec::new();
        for l in &links {
            match symlink_or_copy(&target, l) {
                Ok(path) => applied_links.push(Value::String(path.to_string_lossy().into_owned())),
                Err(e) => skipped_links.push(json!({
                    "link": l.to_string_lossy(),
                    "error": e.to_string(),
                    "reason": "PATH link directory is not writable; install continues with remaining link dirs",
                })),
            }
        }
        Some(json!({
            "binary": binary.to_string_lossy(),
            "links": applied_links,
            "skipped_links": skipped_links,
        }))
    } else {
        None
    };

    let target_dir = target.parent().map(|p| p.to_string_lossy().into_owned());
    let path_contains = target_dir
        .as_ref()
        .map(|d| path_dirs().contains(d))
        .unwrap_or(false);

    let next_steps = if apply {
        json!([
            "Run `fighorse doctor` to verify the installed CLI.",
            "Use the absolute target path in GUI MCP clients when PATH inheritance is uncertain."
        ])
    } else {
        json!([
            format!("Run `fighorse install binary --apply --source <binary> --target {}` to install the CLI.", target.to_string_lossy()),
            "Use --link-dir or --link-dirs to add command links in PATH directories."
        ])
    };

    Ok(json!({
        "kind": "fighorse.install-binary.v1",
        "apply": apply,
        "source": source,
        "target": target.to_string_lossy(),
        "links": links.iter().map(|l| Value::String(l.to_string_lossy().into_owned())).collect::<Vec<_>>(),
        "path_contains_target_dir": path_contains,
        "path_link_dirs": all_dirs,
        "applied": applied,
        "next_steps": next_steps,
    }))
}

// --- Home / auth / project ---

fn migrate_legacy_config(home: &Path) -> Option<Value> {
    let legacy = config::legacy_config_path();
    let target = home.join("config.json");
    if legacy.exists() && !target.exists() {
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::copy(&legacy, &target).is_ok() {
            set_mode_600(&target);
            return Some(json!({"from": legacy.to_string_lossy(), "to": target.to_string_lossy()}));
        }
    }
    None
}

fn set_mode_600(target: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(target) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(target, perms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = target;
    }
}

/// Create the fighorse home directory structure.
pub fn install_home(home: Option<&str>) -> Result<Value> {
    let home = fighorse_home(home);
    let mut dirs = Vec::new();
    dirs.push(mkdirp(&home)?);
    for sub in [
        "bin",
        "experience",
        "clients",
        "services",
        "skills",
        "logs",
        "runtime",
        "exports",
    ] {
        dirs.push(mkdirp(&home.join(sub))?);
    }
    let readme = home.join("README.md");
    write_text(&readme, "# fighorse Home\n\nThis directory stores local fighorse configuration, global experience, generated MCP client snippets, service files, skills, logs, runtime files, and exported assets.\n\n- Global experience: `experience/global.jsonl`\n- Project experience: `<project>/.fighorse/experience.jsonl` after `fighorse install project`\n- Override home with `FIGHORSE_HOME`.\n")?;

    Ok(json!({
        "kind": "fighorse.install-home.v1",
        "home": home.to_string_lossy(),
        "directories": dirs.iter().map(|d| Value::String(d.to_string_lossy().into_owned())).collect::<Vec<_>>(),
        "files": [readme.to_string_lossy()],
        "migrated_config": migrate_legacy_config(&home),
        "ai_contract": guidance::ai_contract(),
        "experience": experience::store_info(&ScopeOpts::default()),
    }))
}

struct PreparedAuth {
    config_file: PathBuf,
    content: Option<Vec<u8>>,
    report: Value,
}

fn prepare_auth(token: Option<&str>, home: &Path, apply: bool) -> Result<PreparedAuth> {
    let config_file = home.join("config.json");
    let mut current = read_json_object(&config_file);
    let current_token = current
        .get("token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| config::load_config().token);
    let token = token.map(|t| t.trim().to_string());

    if !apply {
        let has = current_token
            .as_deref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false);
        return Ok(PreparedAuth {
            config_file: config_file.clone(),
            content: None,
            report: json!({
                "kind": "fighorse.install-auth.v1",
                "apply": false,
                "config_path": config_file.to_string_lossy(),
                "has_saved_token": has,
                "next_steps": [
                    "Run `fighorse install auth --apply --token <FIGMA_TOKEN>` to persist a Figma token.",
                    "You can also pipe the token on stdin to avoid exposing it in shell history.",
                    "MCP clients inherit this saved config through FIGHORSE_HOME."
                ]
            }),
        });
    }

    let effective = match token {
        Some(t) if !t.is_empty() => Some(t),
        _ => current_token.clone().filter(|t| !t.trim().is_empty()),
    };

    match effective {
        None => Ok(PreparedAuth {
            config_file: config_file.clone(),
            content: None,
            report: json!({
                "kind": "fighorse.install-auth.v1",
                "apply": true,
                "ok": false,
                "config_path": config_file.to_string_lossy(),
                "has_saved_token": false,
                "needs_token": true,
                "next_steps": [
                    "Provide a Figma token with `--token`, pipe it on stdin, or run `fighorse auth login`.",
                    "Do not commit tokens; fighorse stores them only in the local user config."
                ]
            }),
        }),
        Some(t) => {
            current.insert("token".into(), Value::String(t));
            Ok(PreparedAuth {
                config_file: config_file.clone(),
                content: Some(serde_json::to_vec_pretty(&Value::Object(current))?),
                report: json!({
                    "kind": "fighorse.install-auth.v1",
                    "apply": true,
                    "ok": true,
                    "config_path": config_file.to_string_lossy(),
                    "has_saved_token": true,
                    "next_steps": ["Run `fighorse doctor` or `fighorse smoke <figma-url>` to verify Figma access."]
                }),
            })
        }
    }
}

/// Persist a Figma token to the local config.
pub fn install_auth(token: Option<&str>, home: Option<&str>, apply: bool) -> Result<Value> {
    let home = fighorse_home(home);
    let prepared = prepare_auth(token, &home, apply)?;
    if let Some(content) = prepared.content.as_deref() {
        let mut transaction = transaction::InstallTransaction::new(&home)?;
        transaction.write_managed_with_mode(&prepared.config_file, content, 0o600)?;
        transaction.commit(None)?;
    }
    Ok(prepared.report)
}

/// Remove only the saved token while preserving all unknown configuration.
pub fn logout_auth(home: Option<&str>) -> Result<Value> {
    let home = fighorse_home(home);
    let config_file = home.join("config.json");
    let mut current = read_json_object(&config_file);
    let removed = current.remove("token").is_some();
    let mut transaction = transaction::InstallTransaction::new(&home)?;
    if current.is_empty() {
        transaction.remove_managed(&config_file)?;
    } else {
        let content = serde_json::to_vec_pretty(&Value::Object(current))?;
        transaction.write_managed_with_mode(&config_file, &content, 0o600)?;
    }
    transaction.commit(None)?;
    Ok(json!({
        "kind": "fighorse.auth-logout.v2",
        "removed_token": removed,
        "config_path": config_file,
    }))
}

/// Initialize project-scoped experience.
pub fn install_project(project_dir: Option<&str>) -> Result<Value> {
    let project_dir = project_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let home = config::fighorse_home();
    let mut transaction = transaction::InstallTransaction::new(&home)?;
    let report = install_project_in_transaction(&mut transaction, &project_dir)?;
    transaction.commit(None)?;
    Ok(report)
}

fn install_project_in_transaction(
    transaction: &mut transaction::InstallTransaction,
    project_dir: &Path,
) -> Result<Value> {
    let dir = project_dir.join(".fighorse");
    let config_file = dir.join("fighorse.json");
    let ignore_file = dir.join(".gitignore");
    let readme_file = dir.join("README.md");

    let must_obey = guidance::ai_contract()
        .get("must")
        .cloned()
        .unwrap_or(Value::Array(vec![]));

    let managed_config = json!({
        "kind": "fighorse.project.v1",
        "schema_version": 1,
        "experience": {
            "scope": "project",
            "write_path": ".fighorse/experience.jsonl",
            "read_order": ["project", "global"],
            "compatibility": "Experience records are append-only JSONL; readers ignore unknown fields."
        },
        "exports": {"scratch": ".fighorse/exports", "packaged": "assets/fighorse", "manifest_required": true},
        "ai": {
            "default_workflow": ["discover_fighorse", "list_experiences", "get_design_package", "visual_audit", "record_experience"],
            "must_obey": must_obey,
            "ask_when_missing": ["platform", "asset_format"]
        }
    });
    let mut config = read_json_object(&config_file);
    for (key, value) in managed_config
        .as_object()
        .expect("managed project config is an object")
    {
        config.insert(key.clone(), value.clone());
    }
    let config_content = serde_json::to_vec_pretty(&Value::Object(config))?;
    transaction.write_managed(&config_file, &config_content)?;
    let ignore_content = merge_managed_block(
        std::fs::read_to_string(&ignore_file).ok().as_deref(),
        "# >>> fighorse managed >>>",
        "# <<< fighorse managed <<<",
        "experience*.jsonl\nexports/\nlogs/\nruntime/",
    );
    transaction.write_managed(&ignore_file, ignore_content.as_bytes())?;
    let readme_content = merge_managed_block(
        std::fs::read_to_string(&readme_file).ok().as_deref(),
        "<!-- >>> fighorse managed >>>",
        "<!-- <<< fighorse managed <<< -->",
        "# fighorse Project\n\nThis project is initialized for project-scoped fighorse experience.\n\n- Write path: `.fighorse/experience.jsonl`\n- Temporary exports: `.fighorse/exports`\n- Packaged assets: `assets/fighorse` or the app's normal resource directory\n- Reads merge project experience first and global experience second.\n- Keep `fighorse.json` in source control if the team wants consistent AI behavior.",
    );
    transaction.write_managed(&readme_file, readme_content.as_bytes())?;

    Ok(project_install_report(project_dir, true))
}

fn project_install_report(project_dir: &Path, applied: bool) -> Value {
    let dir = project_dir.join(".fighorse");
    let config_file = dir.join("fighorse.json");
    let ignore_file = dir.join(".gitignore");
    let readme_file = dir.join("README.md");
    let opts = ScopeOpts {
        scope: Some("project".to_string()),
        project_dir: Some(project_dir.to_string_lossy().into_owned()),
    };
    json!({
        "kind": "fighorse.install-project.v1",
        "apply": applied,
        "project_dir": project_dir.to_string_lossy(),
        "files": [config_file.to_string_lossy(), ignore_file.to_string_lossy(), readme_file.to_string_lossy()],
        "ai_contract": guidance::ai_contract(),
        "experience": experience::store_info(&opts),
    })
}

fn merge_managed_block(existing: Option<&str>, start: &str, end: &str, managed: &str) -> String {
    let existing = existing.unwrap_or_default();
    let retained = match (existing.find(start), existing.find(end)) {
        (Some(begin), Some(finish)) if begin <= finish => {
            let suffix = finish + end.len();
            format!("{}{}", &existing[..begin], &existing[suffix..])
        }
        _ => existing.to_string(),
    };
    let retained = retained.trim_end();
    if retained.is_empty() {
        format!("{start}\n{managed}\n{end}\n")
    } else {
        format!("{retained}\n\n{start}\n{managed}\n{end}\n")
    }
}

fn generated_skill_templates() -> skills::GeneratedSkillTemplates {
    skills::GeneratedSkillTemplates::new(skill_markdown(), agents_markdown(), cursor_rule())
        .with_edb26d2_templates()
}

fn canonical_skill_clients(selected: &[String]) -> Vec<ClientKind> {
    let mut clients = Vec::new();
    for client in selected {
        let kind = match ClientKind::parse(client) {
            Ok(kind) => Some(kind),
            Err(_) if client == "generic" => Some(ClientKind::Codex),
            Err(_) => None,
        };
        if let Some(kind) = kind {
            if !clients.contains(&kind) {
                clients.push(kind);
            }
        }
    }
    clients
}

fn apply_skills_in_transaction(
    transaction: &mut transaction::InstallTransaction,
    package_dir: &Path,
    user_home: &Path,
    clients: &[ClientKind],
) -> Result<(Vec<Value>, Vec<Value>, skills::SkillMigrationReport)> {
    let packaged = [
        (package_dir.join("SKILL.md"), skill_markdown()),
        (package_dir.join("AGENTS.md"), agents_markdown()),
        (package_dir.join("cursor-rule.mdc"), cursor_rule()),
    ];
    let mut files = Vec::new();
    for (path, content) in packaged {
        transaction.write_managed(&path, content.as_bytes())?;
        files.push(Value::String(path.to_string_lossy().into_owned()));
    }

    let mut applied = Vec::new();
    for target in skills::canonical_targets(user_home, clients) {
        let content = match target.kind {
            skills::SkillTargetKind::Skill => skill_markdown(),
            skills::SkillTargetKind::CursorRule => cursor_rule(),
        };
        transaction.write_managed(&target.path, content.as_bytes())?;
        applied.push(json!({
            "kind": target.kind,
            "file": target.path.to_string_lossy(),
        }));
    }
    let migration = skills::migrate_legacy_for_clients(
        transaction,
        user_home,
        clients,
        &generated_skill_templates(),
    )?;
    Ok((files, applied, migration))
}

fn apply_skills_transactional(
    package_dir: &Path,
    home: Option<&str>,
    selected: &[String],
) -> Result<(Vec<Value>, Vec<Value>, skills::SkillMigrationReport)> {
    let install_home = fighorse_home(home);
    let mut transaction = transaction::InstallTransaction::new(&install_home)?;
    let canonical_clients = canonical_skill_clients(selected);
    match apply_skills_in_transaction(
        &mut transaction,
        package_dir,
        &home_os(),
        &canonical_clients,
    ) {
        Ok(result) => {
            if let Err(error) = transaction.commit(None) {
                let rollback = transaction.rollback_pending();
                return Err(Error::Other(format!(
                    "{error}; managed rollback: {}",
                    serde_json::to_string(&rollback)?
                )));
            }
            Ok(result)
        }
        Err(error) => {
            let rollback = transaction.rollback_pending();
            Err(Error::Other(format!(
                "{error}; managed rollback: {}",
                serde_json::to_string(&rollback)?
            )))
        }
    }
}

/// Generate (and optionally apply) skill/agent files.
pub fn install_skill(
    dir: Option<&str>,
    home: Option<&str>,
    client: Option<&str>,
    clients: Option<&str>,
    apply: bool,
) -> Result<Value> {
    let base = dir
        .map(PathBuf::from)
        .unwrap_or_else(|| fighorse_home(home).join("skills").join("fighorse"));
    let selected = coerce_clients(client, clients);
    let (files, applied, migration) = if apply {
        let (files, applied, migration) = apply_skills_transactional(&base, home, &selected)?;
        (files, Some(Value::Array(applied)), Some(migration))
    } else {
        (write_skill_set(&base)?, None, None)
    };
    Ok(json!({
        "kind": "fighorse.install-skill.v1",
        "dir": base.to_string_lossy(),
        "files": files,
        "apply": apply,
        "clients": selected,
        "applied": applied,
        "migration": migration,
        "ai_contract": guidance::ai_contract(),
        "usage": [
            "Attach SKILL.md as a skill where supported.",
            "Copy AGENTS.md into an AI coding project when a generic agent instruction file is preferred.",
            "Copy cursor-rule.mdc into .cursor/rules/fighorse.mdc for Cursor project rules.",
            "The generated instructions are intentionally generic across clients; client-specific files are only generated where install behavior is verified.",
            "Use `--apply --clients cursor,codex,kimi,claude` to install the canonical shared skill, Claude skill, and Cursor rule."
        ]
    }))
}

fn merge_json_mcp_config(file: &Path, server: &Value) -> Result<Value> {
    let mut current = read_json_object(file);
    let servers = current
        .entry("mcpServers".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(obj) = servers.as_object_mut() {
        obj.insert("fighorse".to_string(), server.clone());
    }
    write_json_with_backup(file, &Value::Object(current))?;
    Ok(json!({"method": "json-config", "ok": true, "file": file.to_string_lossy()}))
}

fn client_detection(client: &str) -> Value {
    let home = home_os();
    let j = |parts: &[&str]| {
        let mut p = home.clone();
        for part in parts {
            p = p.join(part);
        }
        p.to_string_lossy().into_owned()
    };
    match normalize_client(client).as_str() {
        "cursor" => json!({
            "client": "cursor",
            "command": executable_path("cursor"),
            "mcp_config": j(&[".cursor", "mcp.json"]),
            "skill_dir": j(&[".agents", "skills", "fighorse"]),
            "rule_file": j(&[".cursor", "rules", "fighorse.mdc"]),
            "apply_supported": true,
            "apply_methods": ["cursor --add-mcp", "~/.cursor/mcp.json for Cursor Agent", "cursor agent mcp enable fighorse", "json-config-fallback"],
            "skill_source": "fighorse uses the shared ~/.agents/skills/fighorse canonical skill and a Cursor-specific rule."
        }),
        "codex" => json!({
            "client": "codex",
            "command": executable_path("codex"),
            "mcp_config": j(&[".codex", "config.toml"]),
            "skill_dir": j(&[".agents", "skills", "fighorse"]),
            "apply_supported": true,
            "apply_methods": ["codex mcp add --url", "toml-managed-block-fallback"]
        }),
        "kimi" => json!({
            "client": "kimi",
            "command": executable_path("kimi").or_else(|| executable_path("kimi-cli")),
            "mcp_config": j(&[".kimi", "mcp.json"]),
            "skill_dir": j(&[".agents", "skills", "fighorse"]),
            "apply_supported": true,
            "apply_methods": ["kimi mcp add --transport http", "json-config-fallback"]
        }),
        "generic" => json!({
            "client": "generic",
            "mcp_config": j(&[".config", "agents", "mcp.json"]),
            "skill_dir": j(&[".agents", "skills", "fighorse"]),
            "apply_supported": true,
            "apply_methods": ["json-config"]
        }),
        "claude" => json!({
            "client": "claude",
            "command": executable_path("claude"),
            "mcp_config": j(&[".claude.json"]),
            "skill_dir": j(&[".claude", "skills", "fighorse"]),
            "apply_supported": true,
            "apply_methods": ["claude mcp add --transport http -s user", "json-config-fallback"]
        }),
        "opencode" => json!({
            "client": "opencode",
            "command": executable_path("opencode"),
            "mcp_config": j(&[".config", "opencode", "opencode.json"]),
            "skill_dir": Value::Null,
            "apply_supported": true,
            "apply_methods": ["opencode mcp add --url", "json-config-fallback"]
        }),
        other => json!({
            "client": other,
            "artifact_generation_supported": true,
            "apply_supported": false,
            "reason": "No verified user-level apply strategy for this client yet; generated snippets can still be installed manually or by a client-specific command."
        }),
    }
}

fn apply_canonical_client(spec: &ClientSpec, home: Option<&str>) -> Result<Value> {
    let user_home = home_os();
    let file = clients::config_path(&user_home, spec.kind);
    let existing = std::fs::read_to_string(&file).ok();
    let merged = spec.merge_config(existing.as_deref())?;
    let install_home = fighorse_home(home);
    let mut transaction = transaction::InstallTransaction::new(&install_home)?;
    let outcome = (|| -> Result<(Vec<Value>, skills::SkillMigrationReport)> {
        transaction.write_managed_client_config(&file, merged.as_bytes(), spec)?;
        let mut skill_result = Vec::new();
        for target in skills::canonical_targets(&user_home, &[spec.kind]) {
            let content = match target.kind {
                skills::SkillTargetKind::Skill => skill_markdown(),
                skills::SkillTargetKind::CursorRule => cursor_rule(),
            };
            transaction.write_managed(&target.path, content.as_bytes())?;
            skill_result.push(json!({
                "kind": target.kind,
                "file": target.path.to_string_lossy()
            }));
        }
        let migration = skills::migrate_legacy_for_clients(
            &mut transaction,
            &user_home,
            &[spec.kind],
            &generated_skill_templates(),
        )?;
        Ok((skill_result, migration))
    })();
    let (skill_result, migration) = match outcome {
        Ok(result) => {
            if let Err(error) = transaction.commit(None) {
                let rollback = transaction.rollback_pending();
                return Err(Error::Other(format!(
                    "{error}; managed rollback: {}",
                    serde_json::to_string(&rollback)?
                )));
            }
            result
        }
        Err(error) => {
            let rollback = transaction.rollback_pending();
            return Err(Error::Other(format!(
                "{error}; managed rollback: {}",
                serde_json::to_string(&rollback)?
            )));
        }
    };
    Ok(json!({
        "client": spec.kind.as_str(),
        "ok": true,
        "mcp": {
            "method": if spec.kind == ClientKind::Codex { "toml-config" } else { "json-config" },
            "ok": true,
            "file": file.to_string_lossy()
        },
        "skills": skill_result,
        "migration": migration,
    }))
}

fn apply_client(client: &str, server: &Value, command: &str, home: Option<&str>) -> Result<Value> {
    let client = normalize_client(client);
    if !matches!(
        client.as_str(),
        "cursor" | "codex" | "kimi" | "claude" | "opencode" | "generic"
    ) {
        return Ok(json!({
            "client": client,
            "ok": false,
            "skipped": true,
            "reason": "No verified apply strategy for this client yet; generated reviewable artifacts only."
        }));
    }
    let config_result = match client.as_str() {
        "cursor" => {
            // Cursor's mcp.json expects just {url} for HTTP and {command,args,env}
            // for stdio - it does not use a `transport`/`type` field.
            let payload = cursor_mcp_payload(server)?;
            merge_json_mcp_config(&home_os().join(".cursor").join("mcp.json"), &payload)?
        }
        "codex" => {
            // Prefer codex CLI, fall back to managed TOML block.
            let config_file = home_os().join(".codex").join("config.toml");
            if let Some(codex) = executable_path("codex") {
                let _ = run_command(&codex, &["mcp", "remove", "fighorse"], &[]);
                let add = if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
                    run_command(&codex, &["mcp", "add", "--url", url, "fighorse"], &[])
                } else {
                    run_command(
                        &codex,
                        &[
                            "mcp",
                            "add",
                            "--env",
                            &format!("FIGHORSE_HOME={}", fighorse_home(home).to_string_lossy()),
                            "--env",
                            "FIGHORSE_MCP_MODE=readonly",
                            "--env",
                            "FIGHORSE_MCP_LOCAL_WRITE=deny",
                            "fighorse",
                            "--",
                            command,
                            "mcp",
                            "serve",
                            "--transport",
                            "stdio",
                        ],
                        &[],
                    )
                };
                if add.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    json!({"method": "codex-cli", "ok": true, "result": add})
                } else {
                    let fallback = merge_codex_config(&config_file, server, command, home)?;
                    json!({"method": "codex-cli-with-toml-fallback", "ok": true, "result": add, "fallback": fallback})
                }
            } else {
                merge_codex_config(&config_file, server, command, home)?
            }
        }
        "kimi" => {
            let file = home_os().join(".kimi").join("mcp.json");
            merge_json_mcp_config(&file, server)?
        }
        "claude" => {
            // Claude Code: prefer `claude mcp add -s user`, fall back to
            // merging into ~/.claude.json (mcpServers) with a format transform.
            let config_file = home_os().join(".claude.json");
            if let Some(claude) = executable_path("claude") {
                let add = if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
                    run_command(
                        &claude,
                        &[
                            "mcp",
                            "add",
                            "-s",
                            "user",
                            "--transport",
                            "http",
                            "fighorse",
                            url,
                        ],
                        &[],
                    )
                } else {
                    run_command(
                        &claude,
                        &[
                            "mcp",
                            "add",
                            "-s",
                            "user",
                            "-e",
                            &format!("FIGHORSE_HOME={}", fighorse_home(home).to_string_lossy()),
                            "-e",
                            "FIGHORSE_MCP_MODE=readonly",
                            "-e",
                            "FIGHORSE_MCP_LOCAL_WRITE=deny",
                            "fighorse",
                            "--",
                            command,
                            "mcp",
                            "serve",
                            "--transport",
                            "stdio",
                        ],
                        &[],
                    )
                };
                if add.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    json!({"method": "claude-cli", "ok": true, "result": add})
                } else {
                    let fallback = merge_claude_config(&config_file, server)?;
                    json!({"method": "claude-cli-with-json-fallback", "ok": true, "result": add, "fallback": fallback})
                }
            } else {
                merge_claude_config(&config_file, server)?
            }
        }
        "opencode" => {
            // opencode: prefer `opencode mcp add --url` for remote, fall back
            // to merging into ~/.config/opencode/opencode.json (mcp section).
            let config_file = home_os()
                .join(".config")
                .join("opencode")
                .join("opencode.json");
            if let Some(opencode) = executable_path("opencode") {
                let add = if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
                    run_command(&opencode, &["mcp", "add", "fighorse", "--url", url], &[])
                } else {
                    // Local stdio: opencode mcp add <name> --env K=V ... -- <cmd> <args>
                    run_command(
                        &opencode,
                        &[
                            "mcp",
                            "add",
                            "fighorse",
                            "--env",
                            &format!("FIGHORSE_HOME={}", fighorse_home(home).to_string_lossy()),
                            "--env",
                            "FIGHORSE_MCP_MODE=readonly",
                            "--env",
                            "FIGHORSE_MCP_LOCAL_WRITE=deny",
                            "--",
                            command,
                            "mcp",
                            "serve",
                            "--transport",
                            "stdio",
                        ],
                        &[],
                    )
                };
                if add.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    json!({"method": "opencode-cli", "ok": true, "result": add})
                } else {
                    let fallback = merge_opencode_config(&config_file, server)?;
                    json!({"method": "opencode-cli-with-json-fallback", "ok": true, "result": add, "fallback": fallback})
                }
            } else {
                merge_opencode_config(&config_file, server)?
            }
        }
        "generic" => {
            let file = home_os().join(".config").join("agents").join("mcp.json");
            merge_json_mcp_config(&file, server)?
        }
        _ => unreachable!(),
    };
    let package_dir = fighorse_home(home).join("skills").join("fighorse");
    let (_, skill_result, migration) =
        apply_skills_transactional(&package_dir, home, std::slice::from_ref(&client))?;
    Ok(json!({
        "client": client,
        "ok": config_result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "mcp": config_result,
        "skills": skill_result,
        "migration": migration,
    }))
}

fn payload_transport(server: &Value) -> Result<&str> {
    match server.get("transport").and_then(Value::as_str) {
        Some("http") => Ok("http"),
        Some("sse") => Err(Error::Usage(
            "Legacy SSE transport is retired. Use --transport http and the /mcp endpoint.".into(),
        )),
        Some(other) => Err(Error::Usage(format!(
            "Unknown client transport: {other}. Expected http or explicit stdio."
        ))),
        None if server.get("command").is_some() => Ok("stdio"),
        None => Err(Error::Usage(
            "Client payload must use HTTP or explicit stdio.".into(),
        )),
    }
}

fn payload_url(server: &Value) -> Result<Value> {
    server
        .get("url")
        .cloned()
        .filter(|value| value.is_string())
        .ok_or_else(|| Error::Usage("HTTP client payload requires a URL.".into()))
}

/// Cursor's mcp.json shape: HTTP -> `{url}`, stdio -> `{command, args, env}`.
/// Cursor does not use a `transport`/`type` discriminator field.
fn cursor_mcp_payload(server: &Value) -> Result<Value> {
    match payload_transport(server)? {
        "http" => Ok(json!({"url": payload_url(server)?})),
        "stdio" => {
            let mut payload = serde_json::Map::new();
            for k in ["command", "args", "env"] {
                if let Some(v) = server.get(k) {
                    payload.insert(k.to_string(), v.clone());
                }
            }
            Ok(Value::Object(payload))
        }
        _ => unreachable!("payload transport is validated"),
    }
}

/// Transform a fighorse MCP server config into Claude Code's `mcpServers`
/// shape: HTTP uses `{"type": "http", "url": ...}`, stdio uses
/// `{"type": "stdio", "command": ..., "args": ..., "env": ...}`.
fn claude_mcp_payload(server: &Value) -> Result<Value> {
    match payload_transport(server)? {
        "http" => Ok(json!({"type": "http", "url": payload_url(server)?})),
        "stdio" => {
            let mut payload = serde_json::Map::new();
            payload.insert("type".into(), json!("stdio"));
            if let Some(c) = server.get("command") {
                payload.insert("command".into(), c.clone());
            }
            if let Some(a) = server.get("args") {
                payload.insert("args".into(), a.clone());
            }
            if let Some(e) = server.get("env") {
                payload.insert("env".into(), e.clone());
            }
            Ok(Value::Object(payload))
        }
        _ => unreachable!("payload transport is validated"),
    }
}

/// Merge the fighorse server into Claude Code's `~/.claude.json` mcpServers.
fn merge_claude_config(file: &Path, server: &Value) -> Result<Value> {
    let mut current = read_json_object(file);
    let servers = current
        .entry("mcpServers".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(obj) = servers.as_object_mut() {
        obj.insert("fighorse".to_string(), claude_mcp_payload(server)?);
    }
    write_json_with_backup(file, &Value::Object(current))?;
    Ok(json!({"method": "json-config", "ok": true, "file": file.to_string_lossy()}))
}

/// Transform a fighorse MCP server config into opencode's `mcp` shape:
/// HTTP -> `{"type": "remote", "url": ..., "enabled": true}`,
/// stdio -> `{"type": "local", "command": ..., "args": ..., "env": ..., "enabled": true}`.
fn opencode_mcp_payload(server: &Value) -> Result<Value> {
    match payload_transport(server)? {
        "http" => Ok(json!({
            "type": "remote",
            "url": payload_url(server)?,
            "enabled": true
        })),
        "stdio" => {
            let mut payload = serde_json::Map::new();
            payload.insert("type".into(), json!("local"));
            payload.insert("enabled".into(), json!(true));
            if let Some(c) = server.get("command") {
                payload.insert("command".into(), c.clone());
            }
            if let Some(a) = server.get("args") {
                payload.insert("args".into(), a.clone());
            }
            if let Some(e) = server.get("env") {
                payload.insert("env".into(), e.clone());
            }
            Ok(Value::Object(payload))
        }
        _ => unreachable!("payload transport is validated"),
    }
}

/// Merge the fighorse server into opencode's `opencode.json` mcp section.
fn merge_opencode_config(file: &Path, server: &Value) -> Result<Value> {
    let mut current = read_json_object(file);
    let servers = current
        .entry("mcp".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(obj) = servers.as_object_mut() {
        obj.insert("fighorse".to_string(), opencode_mcp_payload(server)?);
    }
    write_json_with_backup(file, &Value::Object(current))?;
    Ok(json!({"method": "json-config", "ok": true, "file": file.to_string_lossy()}))
}

fn merge_codex_config(
    file: &Path,
    server: &Value,
    command: &str,
    home: Option<&str>,
) -> Result<Value> {
    let block = format!(
        "# BEGIN fighorse managed\n{}# END fighorse managed\n",
        codex_toml(server, command, home)
    );
    let current = std::fs::read_to_string(file).unwrap_or_default();
    let updated = if current.contains("# BEGIN fighorse managed") {
        let re =
            regex::Regex::new(r"(?s)# BEGIN fighorse managed.*?# END fighorse managed\n?").unwrap();
        re.replace(&current, block.as_str()).into_owned()
    } else if current.contains("[mcp_servers.fighorse]") {
        current.clone()
    } else if current.trim().is_empty() {
        block.clone()
    } else {
        format!("{current}\n\n{block}")
    };
    if updated == current {
        Ok(json!({
            "method": "toml-config", "ok": false, "file": file.to_string_lossy(), "skipped": true,
            "reason": "Existing unmarked [mcp_servers.fighorse] block found; not overwriting user-managed config."
        }))
    } else {
        write_text_with_backup(file, &updated)?;
        Ok(json!({"method": "toml-config", "ok": true, "file": file.to_string_lossy()}))
    }
}

/// Generate (and optionally apply) MCP client config.
pub fn install_client(
    client: Option<&str>,
    dir: Option<&str>,
    transport: &str,
    port: i64,
    command: &str,
    home: Option<&str>,
    apply: bool,
) -> Result<Value> {
    let client = normalize_client(client.unwrap_or("generic"));
    if transport == "sse" {
        return Err(Error::Usage(
            "Legacy SSE transport is retired. Use --transport http and the /mcp endpoint.".into(),
        ));
    }
    let base = dir
        .map(PathBuf::from)
        .unwrap_or_else(|| fighorse_home(home).join("clients").join(&client));
    let command = command_path(command, home);
    let endpoint = format!("http://127.0.0.1:{port}/mcp");
    let canonical_spec = ClientKind::parse(&client)
        .ok()
        .map(|kind| {
            ClientSpec::from_transport(kind, transport, &endpoint, &command, fighorse_home(home))
        })
        .transpose()?;
    let server = match canonical_spec.as_ref() {
        Some(spec) => spec.json_payload(),
        None => mcp_server_config(transport, port, &command, home)?,
    };

    let mcp_json = base.join("mcp.json");
    let manifest = base.join("fighorse-client.json");
    let readme = base.join("README.md");

    let mut files = Vec::new();
    files.push(Value::String(
        write_json(&mcp_json, &json!({"mcpServers": {"fighorse": server}}))?
            .to_string_lossy()
            .into_owned(),
    ));
    files.push(Value::String(
        write_json(
            &manifest,
            &json!({
                "kind": "fighorse.client-install.v1",
                "client": client,
                "transport": transport,
                "generated_at": now_iso(),
                "mcp_server": server,
                "detected": client_detection(&client),
                "recommended_tool_order": ["discover_fighorse", "check_fighorse_ready", "list_experiences", "get_design_package", "visual_audit", "record_experience"],
                "ai_contract": guidance::ai_contract(),
                "notes": [
                    "By default this command writes reviewable snippets only.",
                    "Use --apply to install into detected user-level client config and skill/rule locations.",
                    "The default HTTP transport reuses the installed local MCP service, so multiple AI clients do not spawn separate fighorse processes.",
                    "Use `figma-api coverage` or MCP resource `fighorse://coverage` for exact public REST API coverage.",
                    "For Codex, apply prefers `codex mcp add` and falls back to a managed TOML block.",
                    "For Cursor, apply uses `cursor --add-mcp`, writes ~/.cursor/mcp.json for Cursor Agent CLI, and attempts `cursor agent mcp enable fighorse`.",
                    "For Kimi, apply prefers `kimi mcp add` and falls back to ~/.kimi/mcp.json."
                ]
            }),
        )?
        .to_string_lossy()
        .into_owned(),
    ));
    let readme_body = format!(
        "# fighorse {client} install\n\n\
Main MCP config: `mcp.json`.\n\n\
Run with `--apply` to install into detected client config and skill locations.\n\n\
Recommended order: discover_fighorse, check_fighorse_ready, list_experiences, get_design_package, visual_audit, record_experience.\n\
For exact public REST API calls, inspect `fighorse://coverage` or run `fighorse figma-api coverage`.\n\n\
## Complementary: Official Figma MCP\n\n\
For capabilities not exposed by the public Figma REST API, also install the official Figma Remote MCP.\n\n\
- Remote URL: `https://mcp.figma.com/mcp`\n\
- Transport: HTTP (Streamable HTTP)\n\
- Auth: OAuth via your Figma account\n\
- Use for: native canvas writes, Code to Canvas, Code Connect auto-mapping, FigJam generation, Make resources\n\
- Pricing: free during beta; will become usage-based paid (per Figma docs)\n\
- Seat: Full seat required for writes to shared files; Dev seat is read-only outside drafts\n\n\
Both fighorse and the official MCP can coexist in the same client. fighorse handles design-to-code read workflows; official MCP handles canvas mutation and product-only features.\n"
    );
    files.push(Value::String(
        write_text(&readme, &readme_body)?
            .to_string_lossy()
            .into_owned(),
    ));

    // Client-specific extra files.
    match client.as_str() {
        "codex" => {
            files.push(Value::String(
                write_text(
                    &base.join("codex-config.toml"),
                    &canonical_spec
                        .as_ref()
                        .expect("codex is a canonical client")
                        .toml_payload(),
                )?
                .to_string_lossy()
                .into_owned(),
            ));
        }
        "cursor" => {
            files.push(Value::String(
                write_text(&base.join("fighorse.cursor.mdc"), &cursor_rule())?
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
        "kimi" => {
            files.push(Value::String(
                write_text(&base.join("KIMI.md"), &agents_markdown())?
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
        "claude" | "opencode" | "openclaw" | "hermes-agent" | "generic" => {
            files.push(Value::String(
                write_text(&base.join("AGENTS.md"), &agents_markdown())?
                    .to_string_lossy()
                    .into_owned(),
            ));
        }
        _ => {}
    }

    let applied = if apply {
        Some(match canonical_spec.as_ref() {
            Some(spec) => apply_canonical_client(spec, home)?,
            None => apply_client(&client, &server, &command, home)?,
        })
    } else {
        None
    };

    let next_steps = if apply {
        json!([
            "Restart or reload the target AI client if it was already running.",
            "Run the client's MCP list command when available and verify fighorse appears.",
            "Ask the AI client to call discover_fighorse before Figma replication."
        ])
    } else {
        json!([
            "Review generated files.",
            "Run with --apply to install into detected client config and skill locations.",
            "Run the client and verify fighorse tools appear."
        ])
    };

    Ok(json!({
        "kind": "fighorse.install-client.v1",
        "client": client,
        "dir": base.to_string_lossy(),
        "transport": transport,
        "files": files,
        "apply": apply,
        "detected": client_detection(&client),
        "applied": applied,
        "ai_contract": guidance::ai_contract(),
        "next_steps": next_steps,
    }))
}

// --- Service ---

/// Generate (and optionally apply) an auto-start MCP service.
pub fn install_service(
    service: &str,
    port: i64,
    command: &str,
    home: Option<&str>,
    apply: bool,
) -> Result<Value> {
    let home_path = fighorse_home(home);
    let home_str = home_path.to_string_lossy().into_owned();
    let command = command_path(command, home);
    let service = if service == "auto" {
        if cfg!(target_os = "macos") {
            "launchd"
        } else {
            "systemd"
        }
    } else {
        service
    };
    if service == "none" {
        return Ok(json!({
            "kind": "fighorse.install-service.v1",
            "service": "none",
            "transport": "none",
            "port": port,
            "file": Value::Null,
            "apply": false,
            "applied": Value::Null,
            "skipped": true,
            "reason": "CLI-only install mode does not start or configure the MCP service."
        }));
    }

    let dir = home_path.join("services");
    let file = match service {
        "launchd" => dir.join("com.groupultra.fighorse.mcp.plist"),
        _ => dir.join("fighorse-mcp.service"),
    };
    mkdirp(&dir)?;
    let target_service = service_target(service);
    let preserve_allow = [file.as_path(), target_service.as_path()]
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .any(|text| {
            text.contains("FIGHORSE_MCP_LOCAL_WRITE=allow")
                || text.contains("<string>allow</string>")
        });
    let content = if service == "launchd" {
        service::launchd_plist(&command, port, &home_str, preserve_allow)
    } else {
        service::systemd_unit(&command, port, &home_str, preserve_allow)
    };
    write_text(&file, &content)?;

    let applied = if apply {
        Some(apply_service(service, &file)?)
    } else {
        None
    };

    let next_steps = if service == "launchd" {
        json!([
            format!(
                "launchctl bootstrap gui/$(id -u) {}",
                file.to_string_lossy()
            ),
            "launchctl kickstart -k gui/$(id -u)/com.groupultra.fighorse.mcp"
        ])
    } else {
        json!([
            format!(
                "mkdir -p ~/.config/systemd/user && cp {} ~/.config/systemd/user/",
                file.to_string_lossy()
            ),
            "systemctl --user daemon-reload",
            "systemctl --user enable --now fighorse-mcp.service"
        ])
    };

    Ok(json!({
        "kind": "fighorse.install-service.v1",
        "service": service,
        "transport": "http",
        "port": port,
        "file": file.to_string_lossy(),
        "apply": apply,
        "applied": applied,
        "next_steps": next_steps,
    }))
}

pub async fn install_service_async(
    service_name: &str,
    port: i64,
    command: &str,
    home: Option<&str>,
    apply: bool,
) -> Result<Value> {
    if !apply || service_name == "none" {
        return install_service(service_name, port, command, home, false);
    }
    let home = fighorse_home(home);
    let home_string = home.to_string_lossy().into_owned();
    install_home(Some(&home_string))?;
    let manager = service_manager(service_name);
    let command = command_path(command, Some(&home_string));
    let endpoint = format!("http://127.0.0.1:{port}/mcp");
    let generated = home.join("services").join(if manager == "launchd" {
        "com.groupultra.fighorse.mcp.plist"
    } else {
        "fighorse-mcp.service"
    });
    let target = service_target(manager);
    let preserve_allow = [generated.as_path(), target.as_path()]
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .any(|text| {
            text.contains("FIGHORSE_MCP_LOCAL_WRITE=allow")
                || text.contains("<string>allow</string>")
        });
    let rendered = if manager == "launchd" {
        service::launchd_plist(&command, port, &home_string, preserve_allow)
    } else {
        service::systemd_unit(&command, port, &home_string, preserve_allow)
    };
    let mut runner = service::ProcessCommandRunner;
    let state = service::probe_service_state(&mut runner, manager, target.clone())?;
    let mut transaction = transaction::InstallTransaction::new(&home)?;
    transaction.set_endpoint(Some(endpoint.clone()));
    transaction.set_service(Some(state));
    let outcome: Result<(Value, Vec<model::InstallCheck>)> = async {
        transaction.write_managed(&generated, rendered.as_bytes())?;
        transaction.write_managed(&target, rendered.as_bytes())?;
        let activated = service::activate_service(
            &mut runner,
            transaction
                .service()
                .expect("service state set before activation"),
        )?;
        let mut checks = service_result_to_checks(manager, &activated)?;
        checks.extend(
            transaction::wait_for_mcp_ready(&endpoint, 100, std::time::Duration::from_millis(100))
                .await?,
        );
        Ok((activated, checks))
    }
    .await;
    let (activated, mut checks) = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            let rollback = transaction.rollback_pending_with_service(&mut runner, true);
            return Err(Error::Other(format!(
                "{error}; rollback: {}",
                serde_json::to_string(&rollback)?
            )));
        }
    };
    transaction.commit(None)?;
    checks.extend(transaction::verify_manifest(&home)?);
    let ok = checks.iter().all(|check| check.ok);
    if !ok {
        let rollback = committed_file_rollback(&home);
        return Err(Error::Other(format!(
            "Service installation verification failed; rollback: {}",
            serde_json::to_string(&rollback)?
        )));
    }
    let final_transaction = transaction::InstallTransaction::new(&home)?;
    final_transaction.commit(Some(checks.clone()))?;
    Ok(json!({
        "kind": "fighorse.install-service.v2",
        "service": manager,
        "transport": "http",
        "endpoint": endpoint,
        "file": generated,
        "target": target,
        "apply": true,
        "applied": activated,
        "verification": checks,
        "ok": true,
    }))
}

fn apply_service(service: &str, file: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(file)?;
    match service {
        "launchd" => {
            let target = home_os()
                .join("Library")
                .join("LaunchAgents")
                .join("com.groupultra.fighorse.mcp.plist");
            if let Some(parent) = target.parent() {
                mkdirp(parent)?;
            }
            write_text_with_backup(&target, &content)?;
            let uid = run_command("id", &["-u"], &[])
                .get("stdout")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let domain = if uid.is_empty() {
                "gui".to_string()
            } else {
                format!("gui/{uid}")
            };
            let target_str = target.to_string_lossy().into_owned();
            Ok(json!({
                "file": target.to_string_lossy(),
                "bootout": run_command("launchctl", &["bootout", &domain, &target_str], &[]),
                "bootstrap": run_command("launchctl", &["bootstrap", &domain, &target_str], &[]),
                "kickstart": run_command("launchctl", &["kickstart", "-k", &format!("{domain}/com.groupultra.fighorse.mcp")], &[]),
            }))
        }
        "systemd" => {
            let target = home_os()
                .join(".config")
                .join("systemd")
                .join("user")
                .join("fighorse-mcp.service");
            if let Some(parent) = target.parent() {
                mkdirp(parent)?;
            }
            write_text_with_backup(&target, &content)?;
            Ok(json!({
                "file": target.to_string_lossy(),
                "daemon_reload": run_command("systemctl", &["--user", "daemon-reload"], &[]),
                "enable_now": run_command("systemctl", &["--user", "enable", "--now", "fighorse-mcp.service"], &[]),
            }))
        }
        other => Ok(
            json!({"skipped": true, "reason": format!("No apply strategy for service manager: {other}")}),
        ),
    }
}

// --- install guide / self / all / status ---

fn install_guide(
    source: Option<&str>,
    path: Option<&str>,
    target: Option<&str>,
    home: Option<&str>,
    clients: &[String],
    mode: &str,
) -> Value {
    let home_path = fighorse_home(home);
    let target = absolute_path(target)
        .map(PathBuf::from)
        .unwrap_or_else(|| install_path_to_target(path, home));
    let source = absolute_path(source).unwrap_or_else(current_executable_path);

    json!({
        "kind": "fighorse.install-guide.v1",
        "purpose": "AI-readable guide for installing a distributed fighorse binary.",
        "human_first_command": "fighorse quickstart",
        "default_install": {
            "command": "fighorse install --default --apply",
            "target": default_binary_target(home).to_string_lossy(),
            "effect": "Copy this binary into the fighorse home bin directory, generate local config, and install fighorse skills/instructions."
        },
        "custom_path_install": {
            "command": "fighorse install --path <install-dir> --apply",
            "exact_file_command": "fighorse install --target <absolute-target-file> --apply",
            "target": target.to_string_lossy()
        },
        "source_checkout_install": {
            "command": "cargo build --release && ./target/release/fighorse install --default --apply --source ./target/release/fighorse",
            "effect": "Build from source, then run the same self-install flow as a packaged binary."
        },
        "packaging": {
            "default": "cargo build --release",
            "macos_x64": "cargo build --release --target x86_64-apple-darwin",
            "macos_arm64": "cargo build --release --target aarch64-apple-darwin",
            "linux_x64": "cargo build --release --target x86_64-unknown-linux-gnu",
            "linux_arm64": "cargo build --release --target aarch64-unknown-linux-gnu"
        },
        "optional_mcp_service": {
            "command": "fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply",
            "endpoint": "http://127.0.0.1:9449/mcp",
            "order": ["service", "/health", "initialize + tools/list", "clients", "skills", "manifest verification"]
        },
        "canonical_skill_targets": {
            "cursor_kimi_codex": "~/.agents/skills/fighorse/SKILL.md",
            "claude": "~/.claude/skills/fighorse/SKILL.md",
            "cursor_rule": "~/.cursor/rules/fighorse.mdc"
        },
        "transaction": {
            "manifest": "~/.fighorse/install/manifest.json",
            "backups": "~/.fighorse/install/backups/",
            "verify": "fighorse install verify",
            "rollback": "fighorse install rollback",
            "managed_removals": "desired_absent=true"
        },
        "ai_client_instructions": [
            "If the binary is not on PATH, copy it to the target path and use that absolute path in MCP/service config.",
            "After install, run fighorse quickstart --format json or MCP check_fighorse_ready.",
            "If auth.has_token is false, prompt the user to run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN.",
            "Use service mode only when Cursor/Codex/Kimi/Claude or another MCP client should call fighorse directly."
        ],
        "current": {
            "source": source,
            "planned_target": target.to_string_lossy(),
            "clients": clients,
            "mode": if mode.is_empty() { "cli" } else { mode }
        },
        "home": home_path.to_string_lossy(),
    })
}

/// Options for install-self / install-all.
#[derive(Default)]
pub struct InstallOpts<'a> {
    pub source: Option<&'a str>,
    pub path: Option<&'a str>,
    pub target: Option<&'a str>,
    pub default: bool,
    pub client: Option<&'a str>,
    pub clients: Option<&'a str>,
    pub transport: &'a str,
    pub port: i64,
    pub command: &'a str,
    pub home: Option<&'a str>,
    pub token: Option<&'a str>,
    pub mode: Option<&'a str>,
    pub service: &'a str,
    pub link_dir: Option<&'a str>,
    pub link_dirs: Option<&'a str>,
    pub no_service: bool,
    pub apply: bool,
}

fn selected_canonical_clients(opts: &InstallOpts, service_mode: bool) -> Result<Vec<ClientKind>> {
    if !service_mode {
        return Ok(Vec::new());
    }
    coerce_clients(opts.client, opts.clients)
        .iter()
        .map(|client| ClientKind::parse(client))
        .collect()
}

fn service_target(service: &str) -> PathBuf {
    match service {
        "launchd" => home_os()
            .join("Library")
            .join("LaunchAgents")
            .join("com.groupultra.fighorse.mcp.plist"),
        _ => home_os()
            .join(".config")
            .join("systemd")
            .join("user")
            .join("fighorse-mcp.service"),
    }
}

fn service_manager(service: &str) -> &str {
    if service == "auto" {
        if cfg!(target_os = "macos") {
            "launchd"
        } else {
            "systemd"
        }
    } else {
        service
    }
}

fn executable_check(path: &Path) -> model::InstallCheck {
    let exists = path.is_file();
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    };
    #[cfg(not(unix))]
    let executable = exists;
    model::InstallCheck::new("binary", exists && executable, path.to_string_lossy())
}

fn binary_path_check(path: &Path) -> model::InstallCheck {
    let parent_on_path = path
        .parent()
        .map(|parent| path_dirs().iter().any(|item| Path::new(item) == parent))
        .unwrap_or(false);
    let resolved = executable_path("fighorse");
    let resolved_matches_target = resolved.as_deref().is_some_and(|resolved| {
        let resolved = Path::new(resolved);
        resolved == path
            || std::fs::canonicalize(resolved)
                .ok()
                .zip(std::fs::canonicalize(path).ok())
                .is_some_and(|(resolved, target)| resolved == target)
    });
    let discoverable = parent_on_path || resolved_matches_target;
    model::InstallCheck::new(
        "binary_path",
        path.is_file() && discoverable,
        if discoverable {
            "fighorse is discoverable on PATH".to_string()
        } else {
            format!(
                "binary is available by absolute path {}; add its directory to PATH if desired",
                path.display()
            )
        },
    )
}

fn config_permission_check(home: &Path) -> model::InstallCheck {
    let path = home.join("config.json");
    if !path.exists() {
        return model::InstallCheck::new("config_permissions", true, "config absent");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .map(|metadata| metadata.permissions().mode() & 0o777)
            .unwrap_or_default();
        model::InstallCheck::new(
            "config_permissions",
            mode == 0o600,
            format!("{} mode {mode:o}", path.display()),
        )
    }
    #[cfg(not(unix))]
    model::InstallCheck::new("config_permissions", true, path.to_string_lossy())
}

async fn apply_install_transaction(opts: &InstallOpts<'_>, self_install: bool) -> Result<Value> {
    let home = fighorse_home(opts.home);
    let home_string = home.to_string_lossy().into_owned();
    let mode = opts.mode.unwrap_or("cli").to_ascii_lowercase();
    let requested_service = matches!(mode.as_str(), "service" | "mcp" | "all");
    let service_mode = requested_service && !opts.no_service && opts.service != "none";
    if requested_service && opts.transport != "http" {
        return Err(Error::Usage(
            "Shared service installation requires --transport http and the /mcp endpoint.".into(),
        ));
    }
    if opts.transport == "sse" {
        return Err(Error::Usage(
            "Legacy SSE transport is retired. Use --transport http and the /mcp endpoint.".into(),
        ));
    }

    let selected = selected_canonical_clients(opts, service_mode)?;
    let endpoint = format!("http://127.0.0.1:{}/mcp", opts.port);
    let plan = if service_mode {
        model::InstallPlan::service(home.clone(), &endpoint, selected.clone())
    } else {
        model::InstallPlan::cli(home.clone())
    };
    let mut completed = vec![model::InstallStep::Preflight];

    install_home(Some(&home_string))?;

    let source = Some(absolute_path(opts.source).unwrap_or_else(current_executable_path));
    let target = absolute_path(opts.target)
        .map(PathBuf::from)
        .or_else(|| {
            if self_install && opts.default {
                Some(default_binary_target(opts.home))
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            if self_install {
                install_path_to_target(opts.path, opts.home)
            } else {
                default_binary_target(opts.home)
            }
        });
    let command = target.to_string_lossy().into_owned();
    let mut transaction = transaction::InstallTransaction::new(&home)?;
    transaction.set_endpoint(service_mode.then_some(endpoint.clone()));
    let prepared_auth = prepare_auth(opts.token, &home, true)?;
    let auth_report = prepared_auth.report.clone();
    let mut service_runner = service::ProcessCommandRunner;
    let mut service_state: Option<service::ServiceState> = None;
    let mut service_touched = false;
    let user_home = home_os();
    let mut skills_migration = None;
    let mut project_report = Value::Null;
    let links = requested_binary_links(opts.link_dir, opts.link_dirs);
    let skill_clients = if service_mode {
        selected.clone()
    } else {
        vec![ClientKind::Codex]
    };
    completed.push(model::InstallStep::Backup);

    let outcome: Result<(Vec<model::InstallCheck>, Value)> = async {
        let mut stage_checks = Vec::new();
        if let Some(content) = prepared_auth.content.as_deref() {
            transaction.write_managed_with_mode(&prepared_auth.config_file, content, 0o600)?;
        }
        if let Some(source) = source.as_deref() {
            let bytes = std::fs::read(source)?;
            transaction.write_managed_with_mode(&target, &bytes, 0o755)?;
            for link in &links {
                #[cfg(unix)]
                let link_result = transaction.write_managed_symlink(link, &target);
                #[cfg(not(unix))]
                let link_result = transaction.write_managed_with_mode(link, &bytes, 0o755);
                match link_result {
                    Ok(()) => stage_checks.push(model::InstallCheck::new(
                        format!("binary_link:{}", link.display()),
                        true,
                        "installed PATH link",
                    )),
                    Err(Error::Io(error))
                        if error.kind() == std::io::ErrorKind::PermissionDenied =>
                    {
                        stage_checks.push(model::InstallCheck::new(
                            format!("binary_link:{}", link.display()),
                            true,
                            format!("skipped unwritable PATH link: {error}"),
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        completed.push(model::InstallStep::Binary);
        if !self_install {
            let project_dir = opts
                .path
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            project_report = install_project_in_transaction(&mut transaction, &project_dir)?;
        }

        let mut service_result = Value::Null;
        if service_mode {
            let manager = service_manager(opts.service);
            let generated = home.join("services").join(if manager == "launchd" {
                "com.groupultra.fighorse.mcp.plist"
            } else {
                "fighorse-mcp.service"
            });
            let target_service = service_target(manager);
            service_state = Some(service::probe_service_state(
                &mut service_runner,
                manager,
                target_service.clone(),
            )?);
            transaction.set_service(service_state.clone());
            let preserve_allow = [generated.as_path(), target_service.as_path()]
                .iter()
                .filter_map(|path| std::fs::read_to_string(path).ok())
                .any(|text| {
                    text.contains("FIGHORSE_MCP_LOCAL_WRITE=allow")
                        || text.contains("<string>allow</string>")
                });
            let rendered = if manager == "launchd" {
                service::launchd_plist(&command, opts.port, &home_string, preserve_allow)
            } else {
                service::systemd_unit(&command, opts.port, &home_string, preserve_allow)
            };
            transaction.write_managed(&generated, rendered.as_bytes())?;
            transaction.write_managed(&target_service, rendered.as_bytes())?;
            service_touched = true;
            service_result = service::activate_service(
                &mut service_runner,
                service_state.as_ref().expect("service state initialized"),
            )?;
            stage_checks.extend(service_result_to_checks(manager, &service_result)?);
            completed.push(model::InstallStep::Service);

            let ready = transaction::wait_for_mcp_ready(
                &endpoint,
                100,
                std::time::Duration::from_millis(100),
            )
            .await?;
            completed.push(model::InstallStep::HealthReady);
            stage_checks.extend(ready);

            for kind in &selected {
                let spec = ClientSpec::new(*kind, &endpoint);
                let review_dir = home.join("clients").join(kind.as_str());
                let review_file = if *kind == ClientKind::Codex {
                    review_dir.join("config.toml")
                } else {
                    review_dir.join("mcp.json")
                };
                transaction.write_managed(&review_file, spec.review_content()?.as_bytes())?;

                let config = clients::config_path(&home_os(), *kind);
                let existing = std::fs::read_to_string(&config).ok();
                let merged = spec.merge_config(existing.as_deref())?;
                transaction.write_managed_client_config(&config, merged.as_bytes(), &spec)?;
            }
            completed.push(model::InstallStep::Clients);
        }

        let packaged_skill = home.join("skills").join("fighorse");
        let (_, _, migration) = apply_skills_in_transaction(
            &mut transaction,
            &packaged_skill,
            &user_home,
            &skill_clients,
        )?;
        skills_migration = Some(migration);
        completed.push(model::InstallStep::Skills);
        Ok((stage_checks, service_result))
    }
    .await;

    let (mut verification, service_result) = match outcome {
        Ok(result) => result,
        Err(error) => {
            let rollback =
                transaction.rollback_pending_with_service(&mut service_runner, service_touched);
            return Err(Error::Other(format!(
                "{error}; rollback: {}",
                serde_json::to_string(&rollback)?
            )));
        }
    };

    if let Err(error) = transaction.commit(None) {
        let rollback =
            transaction.rollback_pending_with_service(&mut service_runner, service_touched);
        return Err(Error::Other(format!(
            "{error}; rollback: {}",
            serde_json::to_string(&rollback)?
        )));
    }
    match transaction::verify_manifest(&home) {
        Ok(checks) => verification.extend(checks),
        Err(error) => {
            let rollback = committed_file_rollback(&home);
            return Err(Error::Other(format!(
                "{error}; rollback: {}",
                serde_json::to_string(&rollback)?
            )));
        }
    }
    verification.push(executable_check(&target));
    verification.push(binary_path_check(&target));
    verification.push(config_permission_check(&home));
    let ok = verification.iter().all(|check| check.ok);
    let final_transaction = match transaction::InstallTransaction::new(&home) {
        Ok(transaction) => transaction,
        Err(error) => {
            let rollback = committed_file_rollback(&home);
            return Err(Error::Other(format!(
                "{error}; rollback: {}",
                serde_json::to_string(&rollback)?
            )));
        }
    };
    if let Err(error) = final_transaction.commit(Some(verification.clone())) {
        let rollback = committed_file_rollback(&home);
        return Err(Error::Other(format!(
            "{error}; rollback: {}",
            serde_json::to_string(&rollback)?
        )));
    }
    completed.push(model::InstallStep::Verified);
    let report = model::InstallReport {
        plan: Some(plan),
        completed,
        verification,
        rollback: Vec::new(),
        skills_migration,
        ok,
    };
    if !ok {
        let rollback = committed_file_rollback(&home);
        return Err(Error::Other(format!(
            "Installation verification failed; rollback: {}",
            serde_json::to_string(&rollback)?
        )));
    }
    Ok(json!({
        "kind": if self_install { "fighorse.install-self.v2" } else { "fighorse.install-all.v2" },
        "apply": true,
        "mode": mode,
        "manifest": transaction::manifest_path(&home),
        "auth": auth_report,
        "project": project_report,
        "service": service_result,
        "report": report,
    }))
}

fn committed_file_rollback(home: &Path) -> Vec<model::InstallCheck> {
    match transaction::rollback(home) {
        Ok(report) => report.rollback,
        Err(error) => vec![model::InstallCheck::new(
            "managed_rollback",
            false,
            error.to_string(),
        )],
    }
}

fn service_result_to_checks(manager: &str, result: &Value) -> Result<Vec<model::InstallCheck>> {
    let fields: &[(&str, &str)] = if manager == "launchd" {
        &[
            ("bootstrap", "service_activation_launchd_bootstrap"),
            ("kickstart", "service_activation_launchd_kickstart"),
        ]
    } else {
        &[
            ("daemon_reload", "service_activation_systemd_daemon_reload"),
            ("enable_now", "service_activation_systemd_enable"),
        ]
    };
    let checks: Vec<_> = fields
        .iter()
        .map(|(field, name)| {
            let value = result.get(*field).cloned().unwrap_or(Value::Null);
            model::InstallCheck::new(
                *name,
                value.get("ok").and_then(Value::as_bool) == Some(true),
                value.to_string(),
            )
        })
        .collect();
    if checks.iter().any(|check| !check.ok) {
        Err(Error::Other(format!(
            "service activation reported failed command: {}",
            serde_json::to_string(&checks)?
        )))
    } else {
        Ok(checks)
    }
}

/// Async production entry point. The CLI uses this boundary so MCP readiness
/// never creates a nested runtime.
pub async fn install_self_async(opts: &InstallOpts<'_>) -> Result<Value> {
    if opts.apply {
        apply_install_transaction(opts, true).await
    } else {
        install_self(opts)
    }
}

/// Async production entry point for a complete setup.
pub async fn install_all_async(opts: &InstallOpts<'_>) -> Result<Value> {
    if opts.apply {
        apply_install_transaction(opts, false).await
    } else {
        install_all(opts)
    }
}

/// Verify the persisted manifest, binary, permissions, and (when installed)
/// the service health and MCP handshake.
pub async fn install_verify(home: Option<&str>, port: i64) -> Result<Value> {
    let home = fighorse_home(home);
    let manifest = transaction::load_manifest(&home)?;
    let endpoint = if port > 0 {
        format!("http://127.0.0.1:{port}/mcp")
    } else {
        manifest
            .endpoint
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:9449/mcp".into())
    };
    let mut checks = transaction::verify_manifest(&home)?;
    let binary = manifest
        .managed_files
        .iter()
        .find(|file| file.path.file_name().and_then(|name| name.to_str()) == Some("fighorse"))
        .map(|file| file.path.clone())
        .unwrap_or_else(|| home.join("bin").join("fighorse"));
    checks.push(executable_check(&binary));
    checks.push(binary_path_check(&binary));
    checks.push(config_permission_check(&home));
    let has_service = manifest.managed_files.iter().any(|file| {
        matches!(
            file.path
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("plist" | "service")
        )
    });
    if has_service {
        match transaction::wait_for_mcp_ready(&endpoint, 1, std::time::Duration::ZERO).await {
            Ok(service_checks) => checks.extend(service_checks),
            Err(error) => checks.push(model::InstallCheck::new(
                "service_health_and_mcp",
                false,
                error.to_string(),
            )),
        }
    }
    checks.push(model::InstallCheck::new(
        "manifest",
        true,
        transaction::manifest_path(&home).to_string_lossy(),
    ));
    let ok = checks.iter().all(|check| check.ok);
    let update = transaction::InstallTransaction::new(&home)?;
    update.commit(Some(checks.clone()))?;
    Ok(serde_json::to_value(model::InstallReport {
        verification: checks,
        ok,
        ..model::InstallReport::default()
    })?)
}

/// Restore only files still matching the managed hashes in the manifest.
pub fn install_rollback(home: Option<&str>) -> Result<Value> {
    Ok(serde_json::to_value(transaction::rollback(
        &fighorse_home(home),
    )?)?)
}

/// Self-install this binary and emit AI-readable install guidance.
pub fn install_self(opts: &InstallOpts) -> Result<Value> {
    let home = fighorse_home(opts.home);
    let home_str = home.to_string_lossy().into_owned();
    let source = absolute_path(opts.source).unwrap_or_else(current_executable_path);
    let target = absolute_path(opts.target)
        .map(PathBuf::from)
        .or_else(|| {
            if opts.default {
                Some(default_binary_target(opts.home))
            } else {
                None
            }
        })
        .unwrap_or_else(|| install_path_to_target(opts.path, opts.home));
    let target_str = target.to_string_lossy().into_owned();
    let mode = opts.mode.unwrap_or("cli").to_lowercase();
    let mcp_mode = matches!(mode.as_str(), "service" | "mcp" | "all");
    let selected = if mcp_mode {
        coerce_clients(opts.client, opts.clients)
    } else {
        vec![]
    };
    let command = if opts.apply {
        target_str.clone()
    } else {
        opts.command.to_string()
    };

    let clients_result: Vec<Value> = selected
        .iter()
        .map(|c| {
            install_client(
                Some(c),
                None,
                opts.transport,
                opts.port,
                &command,
                Some(&home_str),
                opts.apply,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let mut next_steps = vec![
        Value::String("Run `fighorse quickstart` to verify setup.".into()),
        Value::String(
            "Run `fighorse auth login --token <FIGMA_TOKEN>` before calling Figma APIs.".into(),
        ),
    ];
    if !opts.apply {
        next_steps.push(Value::String(
            "Add --apply to copy this binary and install generated config.".into(),
        ));
    }
    if mcp_mode {
        next_steps.push(Value::String(
            "Restart or reload Cursor/Codex/Kimi/Claude and ask it to call discover_fighorse."
                .into(),
        ));
    }

    Ok(json!({
        "kind": "fighorse.install-self.v1",
        "apply": opts.apply,
        "mode": mode,
        "source": source,
        "target": target_str,
        "guide": install_guide(Some(&source), opts.path, Some(&target_str), Some(&home_str), &selected, &mode),
        "home": install_home(Some(&home_str))?,
        "auth": install_auth(opts.token, Some(&home_str), opts.apply)?,
        "binary": install_binary(Some(&source), Some(&target_str), opts.link_dir, opts.link_dirs, Some(&home_str), opts.apply)?,
        "skill": install_skill(None, Some(&home_str), None, opts.clients.filter(|_| mcp_mode), opts.apply)?,
        "clients": selected,
        "clients_result": clients_result,
        "service": install_service(if mcp_mode { opts.service } else { "none" }, opts.port, &command, Some(&home_str), opts.apply)?,
        "next_steps": next_steps,
    }))
}

/// Generate or apply a full setup (default CLI-only).
pub fn install_all(opts: &InstallOpts) -> Result<Value> {
    let home = fighorse_home(opts.home);
    let home_str = home.to_string_lossy().into_owned();
    let mode = opts.mode.unwrap_or("cli").to_lowercase();
    let mcp_mode = matches!(mode.as_str(), "service" | "mcp" | "all");
    let skip_service = !mcp_mode || opts.no_service || opts.service == "none";
    let selected = if mcp_mode {
        coerce_clients(opts.client, opts.clients)
    } else {
        vec![]
    };
    let binary_target = absolute_path(opts.target)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_binary_target(opts.home));
    let binary_target_str = binary_target.to_string_lossy().into_owned();
    let has_source = opts.source.map(|s| !s.trim().is_empty()).unwrap_or(false);
    let command = if opts.apply && has_source {
        binary_target_str.clone()
    } else {
        command_path(opts.command, opts.home)
    };
    let project_dir = opts
        .path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let clients_result: Vec<Value> = selected
        .iter()
        .map(|c| {
            install_client(
                Some(c),
                None,
                opts.transport,
                opts.port,
                &command,
                Some(&home_str),
                opts.apply,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(json!({
        "kind": "fighorse.install-all.v1",
        "apply": opts.apply,
        "mode": mode,
        "clients": selected,
        "home": install_home(Some(&home_str))?,
        "auth": install_auth(opts.token, Some(&home_str), opts.apply)?,
        "binary": install_binary(opts.source, Some(&binary_target_str), opts.link_dir, opts.link_dirs, Some(&home_str), opts.apply && has_source)?,
        "project": project_install_report(&project_dir, false),
        "skill": install_skill(None, Some(&home_str), None, opts.clients.filter(|_| mcp_mode), opts.apply)?,
        "clients_result": clients_result,
        "service": install_service(if skip_service { "none" } else { opts.service }, opts.port, &command, Some(&home_str), opts.apply)?,
    }))
}

fn active_pid(pid: Option<i64>) -> bool {
    match pid {
        Some(p) if p > 0 => {
            #[cfg(unix)]
            unsafe {
                extern "C" {
                    fn kill(pid: i32, sig: i32) -> i32;
                }
                kill(p as i32, 0) == 0 || std::io::Error::last_os_error().raw_os_error() != Some(3)
            }
            #[cfg(not(unix))]
            {
                false
            }
        }
        _ => false,
    }
}

/// Show install paths and detected state.
pub fn status() -> Value {
    let home = config::fighorse_home();
    let mut seen = std::collections::HashSet::new();
    let normalized_clients: Vec<String> = SUPPORTED_CLIENTS
        .iter()
        .map(|c| normalize_client(c))
        .filter(|c| seen.insert(c.clone()))
        .collect();
    let lock_file = home.join("runtime").join("mcp.lock");
    let lock_present = lock_file.exists();
    let lock = if lock_present {
        Some(read_json_object(&lock_file))
    } else {
        None
    };
    let lock_pid = lock
        .as_ref()
        .and_then(|l| l.get("pid"))
        .and_then(|v| v.as_i64());
    let active = active_pid(lock_pid);

    let mut detected = Map::new();
    for c in &normalized_clients {
        detected.insert(c.clone(), client_detection(c));
    }

    json!({
        "kind": "fighorse.install-status.v1",
        "home": home.to_string_lossy(),
        "home_exists": home.exists(),
        "platform": std::env::consts::OS,
        "binary": {
            "default_target": default_binary_target(None).to_string_lossy(),
            "current_fighorse": executable_path("fighorse"),
            "path_dirs": path_dirs(),
            "preferred_link_dirs": path_preferred_link_dirs()
        },
        "experience": experience::store_info(&ScopeOpts::default()),
        "clients_dir": home.join("clients").to_string_lossy(),
        "services_dir": home.join("services").to_string_lossy(),
        "skills_dir": home.join("skills").to_string_lossy(),
        "public_quickstart": {
            "default_install_mode": "cli",
            "cli_install": "fighorse install --default --apply",
            "source_cli_install": "cargo build --release && ./target/release/fighorse install --default --apply --source ./target/release/fighorse",
            "service_install": "fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply",
            "first_check": "fighorse quickstart \"<figma-frame-url>\""
        },
        "canonical_skill_targets": {
            "cursor_kimi_codex": "~/.agents/skills/fighorse/SKILL.md",
            "claude": "~/.claude/skills/fighorse/SKILL.md",
            "cursor_rule": "~/.cursor/rules/fighorse.mdc"
        },
        "transaction": {
            "manifest": "~/.fighorse/install/manifest.json",
            "backups": "~/.fighorse/install/backups/",
            "verify": "fighorse install verify",
            "rollback": "fighorse install rollback",
            "managed_removals": "desired_absent=true"
        },
        "mcp_service": {
            "endpoint": "http://127.0.0.1:9449/mcp",
            "health": "http://127.0.0.1:9449/health",
            "lock_file": lock_file.to_string_lossy(),
            "lock_present": lock_present,
            "pid": lock_pid,
            "running": active,
            "stale_lock": lock_present && !active,
            "next_step": if active {
                "MCP service appears to be running. Configure clients to use http://127.0.0.1:9449/mcp."
            } else {
                "If an AI client needs MCP, install explicit service mode; CLI-only users do not need this service."
            }
        },
        "troubleshooting": {
            "token_missing": "Run fighorse auth login --token <FIGMA_TOKEN>.",
            "client_config": "Generated client configs use http://127.0.0.1:9449/mcp and expect one shared local service.",
            "codex_handshake": "Repeated /mcp initialize requests must return a standard Streamable HTTP JSON or event-stream response, not a product manifest; restart the service after upgrading.",
            "local_write": "MCP exports require FIGHORSE_MCP_LOCAL_WRITE=allow and an approved export root.",
            "stale_lock": format!("Remove {} only after confirming no fighorse MCP service is running.", lock_file.to_string_lossy())
        },
        "supported_clients": SUPPORTED_CLIENTS,
        "detected_clients": Value::Object(detected),
        "apply_note": "Commands are dry-run/artifact-generating by default; pass --apply to mutate detected client configs, skill locations, and service managers."
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_coerce_clients() {
        assert_eq!(normalize_client("kimi-cli"), "kimi");
        assert_eq!(normalize_client("unknown"), "generic");
        assert_eq!(coerce_clients(Some("cursor"), None), vec!["cursor"]);
        assert_eq!(
            coerce_clients(None, Some("cursor,codex,cursor")),
            vec!["cursor", "codex"]
        );
        assert!(coerce_clients(None, Some("none")).is_empty());
    }

    #[test]
    fn install_home_creates_dirs() {
        let tmp = std::env::temp_dir().join(format!("fh-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let result = install_home(Some(&tmp.to_string_lossy())).unwrap();
        assert_eq!(result["kind"], "fighorse.install-home.v1");
        assert!(tmp.join("bin").exists());
        assert!(tmp.join("experience").exists());
        assert!(tmp.join("README.md").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_client_generates_files() {
        let tmp = std::env::temp_dir().join(format!("fh-client-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let result = install_client(
            Some("cursor"),
            Some(&tmp.to_string_lossy()),
            "http",
            9449,
            "fighorse",
            None,
            false,
        )
        .unwrap();
        assert_eq!(result["client"], "cursor");
        assert!(tmp.join("mcp.json").exists());
        assert!(tmp.join("fighorse.cursor.mdc").exists());
        let mcp: Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.join("mcp.json")).unwrap()).unwrap();
        assert_eq!(
            mcp["mcpServers"]["fighorse"]["url"],
            "http://127.0.0.1:9449/mcp"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_service_systemd_unit() {
        let tmp = std::env::temp_dir().join(format!("fh-svc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let result = install_service(
            "systemd",
            9449,
            "fighorse",
            Some(&tmp.to_string_lossy()),
            false,
        )
        .unwrap();
        assert_eq!(result["service"], "systemd");
        let file = result["file"].as_str().unwrap();
        let content = std::fs::read_to_string(file).unwrap();
        assert!(content.contains("ExecStart="));
        assert!(content.contains("mcp serve --transport http"));
        assert!(content.contains("FIGHORSE_MCP_LOCAL_WRITE=deny"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn claude_mcp_payload_accepts_http_and_explicit_stdio_but_rejects_sse() {
        // HTTP: transport -> type, url preserved.
        let http = json!({"transport": "http", "url": "http://127.0.0.1:9449/mcp"});
        let p = claude_mcp_payload(&http).unwrap();
        assert_eq!(p["type"], "http");
        assert_eq!(p["url"], "http://127.0.0.1:9449/mcp");
        assert!(p.get("transport").is_none());

        let sse = json!({"transport": "sse", "url": "http://127.0.0.1:9449/sse"});
        let error = claude_mcp_payload(&sse).unwrap_err().to_string();
        assert!(error.contains("--transport http"));
        assert!(error.contains("/mcp"));

        // stdio: no transport field -> type=stdio, command/args/env preserved.
        let stdio = json!({
            "command": "/path/to/fighorse",
            "args": ["mcp", "serve", "--transport", "stdio"],
            "env": {"FIGHORSE_MCP_MODE": "readonly"}
        });
        let p = claude_mcp_payload(&stdio).unwrap();
        assert_eq!(p["type"], "stdio");
        assert_eq!(p["command"], "/path/to/fighorse");
        assert!(p["args"].is_array());
        assert_eq!(p["env"]["FIGHORSE_MCP_MODE"], "readonly");
    }

    #[test]
    fn opencode_mcp_payload_transforms_all_transports() {
        let http = json!({"transport": "http", "url": "http://127.0.0.1:9449/mcp"});
        let p = opencode_mcp_payload(&http).unwrap();
        assert_eq!(p["type"], "remote");
        assert_eq!(p["url"], "http://127.0.0.1:9449/mcp");
        assert_eq!(p["enabled"], true);

        let stdio = json!({"command": "fighorse", "args": ["mcp", "serve"], "env": {"K": "v"}});
        let p = opencode_mcp_payload(&stdio).unwrap();
        assert_eq!(p["type"], "local");
        assert_eq!(p["enabled"], true);
        assert_eq!(p["command"], "fighorse");
        assert_eq!(p["env"]["K"], "v");

        let sse = json!({"transport": "sse", "url": "http://127.0.0.1:9449/sse"});
        let error = opencode_mcp_payload(&sse).unwrap_err().to_string();
        assert!(error.contains("--transport http"));
        assert!(error.contains("/mcp"));
    }

    #[test]
    fn install_binary_skips_unwritable_link_dirs() {
        let tmp = std::env::temp_dir().join(format!("fh-bin-skip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let home = tmp.join("home");
        let writable = tmp.join("writable-bin");
        let locked = tmp.join("locked-bin");
        std::fs::create_dir_all(&writable).unwrap();
        std::fs::create_dir_all(&locked).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&locked).unwrap().permissions();
            perms.set_mode(0o555);
            std::fs::set_permissions(&locked, perms).unwrap();
        }
        let source = tmp.join("source-fighorse");
        std::fs::write(&source, "#!/bin/sh\necho fighorse\n").unwrap();
        set_executable(&source);
        let link_dirs = format!(
            "{},{}",
            writable.to_string_lossy(),
            locked.to_string_lossy()
        );
        let result = install_binary(
            Some(&source.to_string_lossy()),
            None,
            None,
            Some(&link_dirs),
            Some(&home.to_string_lossy()),
            true,
        )
        .unwrap();
        let applied = result["applied"].as_object().unwrap();
        let links = applied["links"].as_array().unwrap();
        let skipped = applied["skipped_links"].as_array().unwrap();
        assert!(links
            .iter()
            .any(|v| v.as_str().unwrap().contains("writable-bin")));
        assert!(
            skipped
                .iter()
                .any(|v| v["link"].as_str().unwrap().contains("locked-bin")),
            "unwritable link dir should be reported in skipped_links: {skipped:?}"
        );
        assert!(writable.join("fighorse").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&locked).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&locked, perms).unwrap();
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_all_cli_mode_skips_service_and_clients() {
        let tmp = std::env::temp_dir().join(format!("fh-all-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let home = tmp.to_string_lossy().into_owned();
        let opts = InstallOpts {
            transport: "http",
            port: 9449,
            command: "fighorse",
            service: "auto",
            home: Some(&home),
            path: Some(&home), // project dir target for install_project
            ..Default::default()
        };
        let result = install_all(&opts).unwrap();
        assert_eq!(result["mode"], "cli");
        assert!(result["clients"].as_array().unwrap().is_empty());
        assert!(result["clients_result"].as_array().unwrap().is_empty());
        assert_eq!(result["service"]["service"], "none");
        assert_eq!(result["service"]["skipped"], true);
        assert_eq!(result["project"]["apply"], false);
        assert!(!tmp.join(".fighorse/fighorse.json").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_all_service_mode_is_explicit() {
        let tmp = std::env::temp_dir().join(format!("fh-all-svc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let home = tmp.to_string_lossy().into_owned();
        let opts = InstallOpts {
            transport: "http",
            port: 9449,
            command: "fighorse",
            service: "systemd",
            mode: Some("service"),
            clients: Some("cursor,codex"),
            home: Some(&home),
            path: Some(&home),
            ..Default::default()
        };
        let result = install_all(&opts).unwrap();
        assert_eq!(result["mode"], "service");
        assert_eq!(result["clients"], serde_json::json!(["cursor", "codex"]));
        assert_eq!(result["service"]["transport"], "http");
        assert!(result["service"]["file"].is_string());
        assert_eq!(result["project"]["apply"], false);
        assert!(!tmp.join(".fighorse/fighorse.json").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_self_uses_absolute_command_paths() {
        let tmp = std::env::temp_dir().join(format!("fh-self-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let home = tmp.to_string_lossy().into_owned();
        let source = tmp.join("source-fighorse");
        std::fs::write(&source, "#!/bin/sh\necho fighorse\n").unwrap();
        set_executable(&source);
        let install_dir = tmp.join("install-bin");

        let opts = InstallOpts {
            source: Some(&source.to_string_lossy()),
            path: Some(&install_dir.to_string_lossy()),
            home: Some(&home),
            link_dirs: Some("none"),
            transport: "stdio",
            port: 9449,
            command: "fighorse",
            service: "auto",
            apply: true,
            ..Default::default()
        };
        let self_result = install_self(&opts).unwrap();
        let target = self_result["target"].as_str().unwrap();
        assert!(PathBuf::from(target).is_absolute());
        assert_eq!(self_result["kind"], "fighorse.install-self.v1");
        assert_eq!(self_result["guide"]["kind"], "fighorse.install-guide.v1");
        assert!(tmp
            .join("skills")
            .join("fighorse")
            .join("SKILL.md")
            .exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
