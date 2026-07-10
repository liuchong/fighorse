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
    home.map(PathBuf::from).unwrap_or_else(config::fighorse_home)
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

fn mask_token(token: &str) -> Option<String> {
    if token.trim().is_empty() {
        None
    } else {
        let n = token.len().min(6);
        Some(format!("{}...", &token[..n]))
    }
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
            if p.ends_with('/') {
                pb.join("fighorse")
            } else if pb.is_dir() {
                pb.join("fighorse")
            } else if pb.file_name().and_then(|f| f.to_str()) == Some("fighorse") {
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
            "FIGHORSE_MCP_LOCAL_WRITE": "allow",
            "FIGHORSE_HOME": fighorse_home(home).to_string_lossy()
        }
    })
}

fn mcp_server_config(transport: &str, port: i64, command: &str, home: Option<&str>) -> Value {
    let command = command_path(command, home);
    match transport {
        "http" => json!({"transport": "http", "url": format!("http://127.0.0.1:{port}/mcp")}),
        "sse" => json!({"transport": "sse", "url": format!("http://127.0.0.1:{port}/sse")}),
        _ => mcp_stdio_config(&command, home),
    }
}

fn codex_toml(server: &Value, command: &str, home: Option<&str>) -> String {
    if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
        format!(
            "[mcp_servers.fighorse]\nurl = \"{url}\"\nenabled = true\nstartup_timeout_sec = 60\n"
        )
    } else {
        format!(
            "[mcp_servers.fighorse]\ncommand = \"{command}\"\nargs = [\"mcp\", \"serve\", \"--transport\", \"stdio\"]\nenabled = true\nstartup_timeout_sec = 60\n\n[mcp_servers.fighorse.env]\nFIGHORSE_MCP_MODE = \"readonly\"\nFIGHORSE_MCP_LOCAL_WRITE = \"allow\"\nFIGHORSE_HOME = \"{}\"\n",
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
            let code = out.status.code().unwrap_or(if out.status.success() { 0 } else { 1 });
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
        return Err(Error::Other("--source is required when applying binary installation".into()));
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

    let mut requested: Vec<String> = split_list(link_dirs.unwrap_or(""));
    if let Some(ld) = link_dir {
        requested.push(ld.to_string());
    }
    let disable = requested.iter().any(|d| d.trim().to_lowercase() == "none");
    let mut all_dirs = if disable {
        vec![]
    } else {
        let mut v = requested.clone();
        v.extend(path_preferred_link_dirs());
        v
    };
    let mut seen = std::collections::HashSet::new();
    all_dirs = all_dirs
        .into_iter()
        .filter(|d| !d.trim().is_empty())
        .filter_map(|d| absolute_path(Some(&d)))
        .filter(|d| seen.insert(d.clone()))
        .collect();
    let links: Vec<PathBuf> = all_dirs
        .iter()
        .map(|d| PathBuf::from(d).join("fighorse"))
        .collect();

    let applied = if apply {
        let src = source.clone().ok_or_else(|| {
            Error::Other("--source is required when applying binary installation".into())
        })?;
        let binary = copy_executable(&src, &target)?;
        let mut applied_links = Vec::new();
        for l in &links {
            applied_links.push(Value::String(
                symlink_or_copy(&target, l)?.to_string_lossy().into_owned(),
            ));
        }
        Some(json!({
            "binary": binary.to_string_lossy(),
            "links": applied_links,
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
    for sub in ["bin", "experience", "clients", "services", "skills", "logs", "runtime", "exports"] {
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

/// Persist a Figma token to the local config.
pub fn install_auth(token: Option<&str>, home: Option<&str>, apply: bool) -> Result<Value> {
    let home = fighorse_home(home);
    let config_file = home.join("config.json");
    let current_token = read_json_object(&config_file)
        .get("token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| config::load_config().token);
    let token = token.map(|t| t.trim().to_string());

    if !apply {
        let has = current_token.as_deref().map(|t| !t.trim().is_empty()).unwrap_or(false);
        return Ok(json!({
            "kind": "fighorse.install-auth.v1",
            "apply": false,
            "config_path": config_file.to_string_lossy(),
            "has_saved_token": has,
            "token_mask": current_token.as_deref().and_then(mask_token),
            "next_steps": [
                "Run `fighorse install auth --apply --token <FIGMA_TOKEN>` to persist a Figma token.",
                "You can also pipe the token on stdin to avoid exposing it in shell history.",
                "MCP clients inherit this saved config through FIGHORSE_HOME."
            ]
        }));
    }

    let effective = match token {
        Some(t) if !t.is_empty() => Some(t),
        _ => current_token.clone().filter(|t| !t.trim().is_empty()),
    };

    match effective {
        None => Ok(json!({
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
        })),
        Some(t) => {
            mkdirp(&home)?;
            write_json_with_backup(&config_file, &json!({"token": t}))?;
            set_mode_600(&config_file);
            Ok(json!({
                "kind": "fighorse.install-auth.v1",
                "apply": true,
                "ok": true,
                "config_path": config_file.to_string_lossy(),
                "has_saved_token": true,
                "token_mask": mask_token(&t),
                "next_steps": ["Run `fighorse doctor` or `fighorse smoke <figma-url>` to verify Figma access."]
            }))
        }
    }
}

/// Initialize project-scoped experience.
pub fn install_project(project_dir: Option<&str>) -> Result<Value> {
    let project_dir = project_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let dir = project_dir.join(".fighorse");
    let config_file = dir.join("fighorse.json");
    let ignore_file = dir.join(".gitignore");
    let readme_file = dir.join("README.md");

    let must_obey = guidance::ai_contract()
        .get("must")
        .cloned()
        .unwrap_or(Value::Array(vec![]));

    let config = json!({
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

    mkdirp(&dir)?;
    write_json(&config_file, &config)?;
    write_text(&ignore_file, "experience*.jsonl\nexports/\nlogs/\nruntime/\n")?;
    write_text(&readme_file, "# fighorse Project\n\nThis project is initialized for project-scoped fighorse experience.\n\n- Write path: `.fighorse/experience.jsonl`\n- Temporary exports: `.fighorse/exports`\n- Packaged assets: `assets/fighorse` or the app's normal resource directory\n- Reads merge project experience first and global experience second.\n- Keep `fighorse.json` in source control if the team wants consistent AI behavior.\n")?;

    let opts = ScopeOpts {
        scope: Some("project".to_string()),
        project_dir: Some(project_dir.to_string_lossy().into_owned()),
    };
    Ok(json!({
        "kind": "fighorse.install-project.v1",
        "project_dir": project_dir.to_string_lossy(),
        "files": [config_file.to_string_lossy(), ignore_file.to_string_lossy(), readme_file.to_string_lossy()],
        "ai_contract": guidance::ai_contract(),
        "experience": experience::store_info(&opts),
    }))
}

fn client_skill_targets(client: &str) -> Vec<Value> {
    let home = home_os();
    let j = |parts: &[&str]| {
        let mut p = home.clone();
        for part in parts {
            p = p.join(part);
        }
        p.to_string_lossy().into_owned()
    };
    match normalize_client(client).as_str() {
        "cursor" => vec![
            json!({"kind": "skill", "dir": j(&[".cursor", "skills", "fighorse"])}),
            json!({"kind": "rule", "file": j(&[".cursor", "rules", "fighorse.mdc"])}),
        ],
        "codex" => vec![json!({"kind": "skill", "dir": j(&[".codex", "skills", "fighorse"])})],
        "kimi" => vec![json!({"kind": "skill", "dir": j(&[".kimi", "skills", "fighorse"])})],
        "claude" => vec![json!({"kind": "skill", "dir": j(&[".claude", "skills", "fighorse"])})],
        "opencode" => vec![json!({"kind": "skill", "dir": j(&[".config", "opencode", "skills", "fighorse"])})],
        "generic" => vec![
            json!({"kind": "skill", "dir": j(&[".config", "agents", "skills", "fighorse"])}),
            json!({"kind": "skill", "dir": j(&[".agents", "skills", "fighorse"])}),
        ],
        _ => vec![],
    }
}

fn apply_skill_target(target: &Value) -> Result<Value> {
    match target.get("kind").and_then(|v| v.as_str()) {
        Some("rule") => {
            let file = PathBuf::from(target.get("file").and_then(|v| v.as_str()).unwrap_or(""));
            write_text_with_backup(&file, &cursor_rule())?;
            Ok(json!({"kind": "rule", "file": file.to_string_lossy()}))
        }
        _ => {
            let dir = PathBuf::from(target.get("dir").and_then(|v| v.as_str()).unwrap_or(""));
            let files = write_skill_set(&dir)?;
            Ok(json!({"kind": "skill", "dir": dir.to_string_lossy(), "files": files}))
        }
    }
}

fn apply_skills(clients: &[String]) -> Result<Vec<Value>> {
    let mut targets = vec![json!({
        "kind": "skill",
        "dir": home_os().join(".config").join("agents").join("skills").join("fighorse").to_string_lossy()
    })];
    for c in clients {
        targets.extend(client_skill_targets(c));
    }
    // distinct by serialized form.
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for t in targets {
        let key = t.to_string();
        if seen.insert(key) {
            out.push(apply_skill_target(&t)?);
        }
    }
    Ok(out)
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
    let files = write_skill_set(&base)?;
    let selected = coerce_clients(client, clients);
    let applied = if apply {
        Some(Value::Array(apply_skills(&selected)?))
    } else {
        None
    };
    Ok(json!({
        "kind": "fighorse.install-skill.v1",
        "dir": base.to_string_lossy(),
        "files": files,
        "apply": apply,
        "clients": selected,
        "applied": applied,
        "ai_contract": guidance::ai_contract(),
        "usage": [
            "Attach SKILL.md as a skill where supported.",
            "Copy AGENTS.md into an AI coding project when a generic agent instruction file is preferred.",
            "Copy cursor-rule.mdc into .cursor/rules/fighorse.mdc for Cursor project rules.",
            "The generated instructions are intentionally generic across clients; client-specific files are only generated where install behavior is verified.",
            "Use `--apply --clients cursor,codex,kimi` to install known user-level skills/rules."
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
            "skill_dir": j(&[".cursor", "skills", "fighorse"]),
            "rule_file": j(&[".cursor", "rules", "fighorse.mdc"]),
            "apply_supported": true,
            "apply_methods": ["cursor --add-mcp", "~/.cursor/mcp.json for Cursor Agent", "cursor agent mcp enable fighorse", "json-config-fallback"],
            "skill_source": "Cursor create-skill documents personal skills at ~/.cursor/skills/<skill-name>; ~/.cursor/skills-cursor is internal."
        }),
        "codex" => json!({
            "client": "codex",
            "command": executable_path("codex"),
            "mcp_config": j(&[".codex", "config.toml"]),
            "skill_dir": j(&[".codex", "skills", "fighorse"]),
            "apply_supported": true,
            "apply_methods": ["codex mcp add --url", "toml-managed-block-fallback"]
        }),
        "kimi" => json!({
            "client": "kimi",
            "command": executable_path("kimi").or_else(|| executable_path("kimi-cli")),
            "mcp_config": j(&[".kimi", "mcp.json"]),
            "skill_dir": j(&[".kimi", "skills", "fighorse"]),
            "apply_supported": true,
            "apply_methods": ["kimi mcp add --transport http", "json-config-fallback"]
        }),
        "generic" => json!({
            "client": "generic",
            "mcp_config": j(&[".config", "agents", "mcp.json"]),
            "skill_dir": j(&[".config", "agents", "skills", "fighorse"]),
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
            "skill_dir": j(&[".config", "opencode", "skills", "fighorse"]),
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

fn apply_client(client: &str, server: &Value, command: &str, home: Option<&str>) -> Result<Value> {
    let client = normalize_client(client);
    if !matches!(client.as_str(), "cursor" | "codex" | "kimi" | "claude" | "opencode" | "generic") {
        return Ok(json!({
            "client": client,
            "ok": false,
            "skipped": true,
            "reason": "No verified apply strategy for this client yet; generated reviewable artifacts only."
        }));
    }
    let config_result = match client.as_str() {
        "cursor" => {
            // Cursor's mcp.json expects just {url} for HTTP/SSE and {command,args,env}
            // for stdio - it does not use a `transport`/`type` field.
            let payload = cursor_mcp_payload(server);
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
                            "mcp", "add", "--env",
                            &format!("FIGHORSE_HOME={}", fighorse_home(home).to_string_lossy()),
                            "--env", "FIGHORSE_MCP_MODE=readonly",
                            "--env", "FIGHORSE_MCP_LOCAL_WRITE=allow",
                            "fighorse", "--", command, "mcp", "serve", "--transport", "stdio",
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
                    run_command(&claude, &["mcp", "add", "-s", "user", "--transport", "http", "fighorse", url], &[])
                } else {
                    run_command(
                        &claude,
                        &[
                            "mcp", "add", "-s", "user",
                            "-e", &format!("FIGHORSE_HOME={}", fighorse_home(home).to_string_lossy()),
                            "-e", "FIGHORSE_MCP_MODE=readonly",
                            "-e", "FIGHORSE_MCP_LOCAL_WRITE=allow",
                            "fighorse", "--", command, "mcp", "serve", "--transport", "stdio",
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
            let config_file = home_os().join(".config").join("opencode").join("opencode.json");
            if let Some(opencode) = executable_path("opencode") {
                let add = if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
                    run_command(&opencode, &["mcp", "add", "fighorse", "--url", url], &[])
                } else {
                    // Local stdio: opencode mcp add <name> --env K=V ... -- <cmd> <args>
                    run_command(
                        &opencode,
                        &[
                            "mcp", "add", "fighorse",
                            "--env", &format!("FIGHORSE_HOME={}", fighorse_home(home).to_string_lossy()),
                            "--env", "FIGHORSE_MCP_MODE=readonly",
                            "--env", "FIGHORSE_MCP_LOCAL_WRITE=allow",
                            "--", command, "mcp", "serve", "--transport", "stdio",
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
    let skill_result = apply_skills(&[client.clone()])?;
    Ok(json!({
        "client": client,
        "ok": config_result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "mcp": config_result,
        "skills": skill_result,
    }))
}

/// Cursor's mcp.json shape: HTTP/SSE -> `{url}`, stdio -> `{command, args, env}`.
/// Cursor does not use a `transport`/`type` discriminator field.
fn cursor_mcp_payload(server: &Value) -> Value {
    let transport = server.get("transport").and_then(|v| v.as_str());
    match transport {
        Some("http" | "sse") => json!({"url": server.get("url").cloned().unwrap_or(Value::Null)}),
        _ => {
            let mut payload = serde_json::Map::new();
            for k in ["command", "args", "env"] {
                if let Some(v) = server.get(k) {
                    payload.insert(k.to_string(), v.clone());
                }
            }
            Value::Object(payload)
        }
    }
}

/// Transform a fighorse MCP server config into Claude Code's `mcpServers`
/// shape: HTTP/SSE use `{"type": ..., "url": ...}`, stdio uses
/// `{"type": "stdio", "command": ..., "args": ..., "env": ...}`.
fn claude_mcp_payload(server: &Value) -> Value {
    let transport = server.get("transport").and_then(|v| v.as_str());
    match transport {
        Some(t @ ("http" | "sse")) => json!({"type": t, "url": server.get("url").cloned().unwrap_or(Value::Null)}),
        // stdio config has command/args/env, no transport field.
        _ => {
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
            Value::Object(payload)
        }
    }
}

/// Merge the fighorse server into Claude Code's `~/.claude.json` mcpServers.
fn merge_claude_config(file: &Path, server: &Value) -> Result<Value> {
    let mut current = read_json_object(file);
    let servers = current
        .entry("mcpServers".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(obj) = servers.as_object_mut() {
        obj.insert("fighorse".to_string(), claude_mcp_payload(server));
    }
    write_json_with_backup(file, &Value::Object(current))?;
    Ok(json!({"method": "json-config", "ok": true, "file": file.to_string_lossy()}))
}

/// Transform a fighorse MCP server config into opencode's `mcp` shape:
/// HTTP/SSE -> `{"type": "remote", "url": ..., "enabled": true}`,
/// stdio -> `{"type": "local", "command": ..., "args": ..., "env": ..., "enabled": true}`.
fn opencode_mcp_payload(server: &Value) -> Value {
    let transport = server.get("transport").and_then(|v| v.as_str());
    match transport {
        Some("http" | "sse") => json!({
            "type": "remote",
            "url": server.get("url").cloned().unwrap_or(Value::Null),
            "enabled": true
        }),
        _ => {
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
            Value::Object(payload)
        }
    }
}

/// Merge the fighorse server into opencode's `opencode.json` mcp section.
fn merge_opencode_config(file: &Path, server: &Value) -> Result<Value> {
    let mut current = read_json_object(file);
    let servers = current
        .entry("mcp".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(obj) = servers.as_object_mut() {
        obj.insert("fighorse".to_string(), opencode_mcp_payload(server));
    }
    write_json_with_backup(file, &Value::Object(current))?;
    Ok(json!({"method": "json-config", "ok": true, "file": file.to_string_lossy()}))
}

fn merge_codex_config(file: &Path, server: &Value, command: &str, home: Option<&str>) -> Result<Value> {
    let block = format!(
        "# BEGIN fighorse managed\n{}# END fighorse managed\n",
        codex_toml(server, command, home)
    );
    let current = std::fs::read_to_string(file).unwrap_or_default();
    let updated = if current.contains("# BEGIN fighorse managed") {
        let re = regex::Regex::new(r"(?s)# BEGIN fighorse managed.*?# END fighorse managed\n?").unwrap();
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
    let base = dir
        .map(PathBuf::from)
        .unwrap_or_else(|| fighorse_home(home).join("clients").join(&client));
    let command = command_path(command, home);
    let server = mcp_server_config(transport, port, &command, home);

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
                write_text(&base.join("codex-config.toml"), &codex_toml(&server, &command, home))?
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
        Some(apply_client(&client, &server, &command, home)?)
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

fn launchd_plist(command: &str, port: &str, home: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key><string>com.groupultra.fighorse.mcp</string>\n  <key>ProgramArguments</key>\n  <array><string>{command}</string><string>mcp</string><string>serve</string><string>--transport</string><string>sse</string><string>--host</string><string>127.0.0.1</string><string>--port</string><string>{port}</string></array>\n  <key>EnvironmentVariables</key>\n  <dict><key>FIGHORSE_HOME</key><string>{home}</string><key>FIGHORSE_MCP_MODE</key><string>readonly</string><key>FIGHORSE_MCP_LOCAL_WRITE</key><string>allow</string></dict>\n  <key>RunAtLoad</key><true/>\n  <key>KeepAlive</key><true/>\n  <key>StandardOutPath</key><string>{home}/logs/mcp.out.log</string>\n  <key>StandardErrorPath</key><string>{home}/logs/mcp.err.log</string>\n</dict>\n</plist>\n"
    )
}

fn systemd_unit(command: &str, port: &str, home: &str) -> String {
    format!(
        "[Unit]\nDescription=fighorse MCP service\n\n[Service]\nEnvironment=FIGHORSE_HOME={home}\nEnvironment=FIGHORSE_MCP_MODE=readonly\nEnvironment=FIGHORSE_MCP_LOCAL_WRITE=allow\nExecStart={command} mcp serve --transport sse --host 127.0.0.1 --port {port}\nRestart=always\nWorkingDirectory={home}\n\n[Install]\nWantedBy=default.target\n"
    )
}

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
    let port_str = port.to_string();

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
    let content = if service == "launchd" {
        launchd_plist(&command, &port_str, &home_str)
    } else {
        systemd_unit(&command, &port_str, &home_str)
    };
    write_text(&file, &content)?;

    let applied = if apply {
        Some(apply_service(service, &file)?)
    } else {
        None
    };

    let next_steps = if service == "launchd" {
        json!([
            format!("launchctl bootstrap gui/$(id -u) {}", file.to_string_lossy()),
            "launchctl kickstart -k gui/$(id -u)/com.groupultra.fighorse.mcp"
        ])
    } else {
        json!([
            format!("mkdir -p ~/.config/systemd/user && cp {} ~/.config/systemd/user/", file.to_string_lossy()),
            "systemctl --user daemon-reload",
            "systemctl --user enable --now fighorse-mcp.service"
        ])
    };

    Ok(json!({
        "kind": "fighorse.install-service.v1",
        "service": service,
        "transport": "http+sse",
        "port": port,
        "file": file.to_string_lossy(),
        "apply": apply,
        "applied": applied,
        "next_steps": next_steps,
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
            let domain = if uid.is_empty() { "gui".to_string() } else { format!("gui/{uid}") };
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
        other => Ok(json!({"skipped": true, "reason": format!("No apply strategy for service manager: {other}")})),
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
            "command": "fighorse install --default --mode service --clients cursor,codex,kimi --apply",
            "endpoint": "http://127.0.0.1:9449/mcp"
        },
        "ai_client_instructions": [
            "If the binary is not on PATH, copy it to the target path and use that absolute path in MCP/service config.",
            "After install, run fighorse quickstart --format json or MCP check_fighorse_ready.",
            "If auth.has_token is false, prompt the user to run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN.",
            "Use service mode only when Cursor/Codex/Kimi or another MCP client should call fighorse directly."
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

/// Self-install this binary and emit AI-readable install guidance.
pub fn install_self(opts: &InstallOpts) -> Result<Value> {
    let home = fighorse_home(opts.home);
    let home_str = home.to_string_lossy().into_owned();
    let source = absolute_path(opts.source).unwrap_or_else(current_executable_path);
    let target = absolute_path(opts.target)
        .map(PathBuf::from)
        .or_else(|| if opts.default { Some(default_binary_target(opts.home)) } else { None })
        .unwrap_or_else(|| install_path_to_target(opts.path, opts.home));
    let target_str = target.to_string_lossy().into_owned();
    let mode = opts.mode.unwrap_or("cli").to_lowercase();
    let mcp_mode = matches!(mode.as_str(), "service" | "mcp" | "all");
    let selected = if mcp_mode {
        coerce_clients(opts.client, opts.clients)
    } else {
        vec![]
    };
    let command = if opts.apply { target_str.clone() } else { opts.command.to_string() };

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
        Value::String("Run `fighorse auth login --token <FIGMA_TOKEN>` before calling Figma APIs.".into()),
    ];
    if !opts.apply {
        next_steps.push(Value::String("Add --apply to copy this binary and install generated config.".into()));
    }
    if mcp_mode {
        next_steps.push(Value::String("Restart or reload Cursor/Codex/Kimi and ask it to call discover_fighorse.".into()));
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
        "project": install_project(opts.path)?,
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
                kill(p as i32, 0) == 0
                    || std::io::Error::last_os_error().raw_os_error() != Some(3)
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
    let lock_pid = lock.as_ref().and_then(|l| l.get("pid")).and_then(|v| v.as_i64());
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
            "service_install": "fighorse install --default --mode service --clients cursor,codex,kimi --apply",
            "first_check": "fighorse quickstart \"<figma-frame-url>\""
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
            "codex_handshake": "Repeated /mcp initialize requests must return MCP JSON/SSE, not text/plain; restart the service after upgrading.",
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
        assert_eq!(coerce_clients(None, Some("cursor,codex,cursor")), vec!["cursor", "codex"]);
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
        let result = install_client(Some("cursor"), Some(&tmp.to_string_lossy()), "http", 9449, "fighorse", None, false).unwrap();
        assert_eq!(result["client"], "cursor");
        assert!(tmp.join("mcp.json").exists());
        assert!(tmp.join("fighorse.cursor.mdc").exists());
        let mcp: Value = serde_json::from_str(&std::fs::read_to_string(tmp.join("mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["fighorse"]["url"], "http://127.0.0.1:9449/mcp");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_service_systemd_unit() {
        let tmp = std::env::temp_dir().join(format!("fh-svc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let result = install_service("systemd", 9449, "fighorse", Some(&tmp.to_string_lossy()), false).unwrap();
        assert_eq!(result["service"], "systemd");
        let file = result["file"].as_str().unwrap();
        let content = std::fs::read_to_string(file).unwrap();
        assert!(content.contains("ExecStart="));
        assert!(content.contains("mcp serve --transport sse"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn claude_mcp_payload_transforms_all_transports() {
        // HTTP: transport -> type, url preserved.
        let http = json!({"transport": "http", "url": "http://127.0.0.1:9449/mcp"});
        let p = claude_mcp_payload(&http);
        assert_eq!(p["type"], "http");
        assert_eq!(p["url"], "http://127.0.0.1:9449/mcp");
        assert!(p.get("transport").is_none());

        // SSE.
        let sse = json!({"transport": "sse", "url": "http://127.0.0.1:9449/sse"});
        let p = claude_mcp_payload(&sse);
        assert_eq!(p["type"], "sse");

        // stdio: no transport field -> type=stdio, command/args/env preserved.
        let stdio = json!({
            "command": "/path/to/fighorse",
            "args": ["mcp", "serve", "--transport", "stdio"],
            "env": {"FIGHORSE_MCP_MODE": "readonly"}
        });
        let p = claude_mcp_payload(&stdio);
        assert_eq!(p["type"], "stdio");
        assert_eq!(p["command"], "/path/to/fighorse");
        assert!(p["args"].is_array());
        assert_eq!(p["env"]["FIGHORSE_MCP_MODE"], "readonly");
    }

    #[test]
    fn opencode_mcp_payload_transforms_all_transports() {
        let http = json!({"transport": "http", "url": "http://127.0.0.1:9449/mcp"});
        let p = opencode_mcp_payload(&http);
        assert_eq!(p["type"], "remote");
        assert_eq!(p["url"], "http://127.0.0.1:9449/mcp");
        assert_eq!(p["enabled"], true);

        let stdio = json!({"command": "fighorse", "args": ["mcp", "serve"], "env": {"K": "v"}});
        let p = opencode_mcp_payload(&stdio);
        assert_eq!(p["type"], "local");
        assert_eq!(p["enabled"], true);
        assert_eq!(p["command"], "fighorse");
        assert_eq!(p["env"]["K"], "v");
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
        assert_eq!(result["service"]["transport"], "http+sse");
        assert!(result["service"]["file"].is_string());
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
        assert!(tmp.join("skills").join("fighorse").join("SKILL.md").exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
