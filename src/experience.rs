//! Local, versioned experience store for AI-assisted Figma replication.
//!
//! Local, versioned experience store. Records are append-only JSONL; reads and
//! filters ignore unknown fields for forward compatibility.

use crate::config;
use crate::error::{Error, Result};
use crate::guidance;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

pub const RECORD_KIND: &str = "fighorse.experience.v1";
pub const SCHEMA_VERSION: i64 = 1;
pub const SUMMARY_KIND: &str = "fighorse.experience-summary.v1";
pub const GUIDANCE_KIND: &str = "fighorse.learned-guidance.v1";

/// Options controlling scope/project resolution.
#[derive(Debug, Clone, Default)]
pub struct ScopeOpts {
    pub scope: Option<String>,
    pub project_dir: Option<String>,
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

pub fn global_experience_path() -> PathBuf {
    config::fighorse_home()
        .join("experience")
        .join("global.jsonl")
}

/// Resolve the current fighorse project directory (falls back to cwd).
pub fn resolve_project_dir(project_dir: Option<&str>) -> PathBuf {
    if let Some(pd) = project_dir {
        return PathBuf::from(pd);
    }
    if let Some(env_dir) = env("FIGHORSE_PROJECT_DIR") {
        return PathBuf::from(env_dir);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.clone();
    loop {
        let project_config = dir.join(".fighorse").join("fighorse.json");
        let git_dir = dir.join(".git");
        if project_config.exists() || git_dir.exists() {
            return dir;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent.to_path_buf(),
            _ => return cwd,
        }
    }
}

pub fn project_config_path(project_dir: Option<&str>) -> PathBuf {
    resolve_project_dir(project_dir)
        .join(".fighorse")
        .join("fighorse.json")
}

pub fn project_experience_path(project_dir: Option<&str>) -> PathBuf {
    resolve_project_dir(project_dir)
        .join(".fighorse")
        .join("experience.jsonl")
}

pub fn project_installed(project_dir: Option<&str>) -> bool {
    project_config_path(project_dir).exists()
}

fn requested_scope(scope: Option<&str>) -> String {
    let value = scope
        .map(String::from)
        .or_else(|| env("FIGHORSE_EXPERIENCE_SCOPE"))
        .unwrap_or_else(|| "auto".to_string());
    let value = value.trim().to_lowercase();
    if value.is_empty() {
        "auto".to_string()
    } else {
        value
    }
}

/// The effective scope: explicit / project / global / merged.
pub fn effective_scope(opts: &ScopeOpts) -> String {
    if env("FIGHORSE_EXPERIENCE_PATH").is_some() {
        return "explicit".to_string();
    }
    let scope = requested_scope(opts.scope.as_deref());
    match scope.as_str() {
        "auto" => {
            if project_installed(opts.project_dir.as_deref()) {
                "project".to_string()
            } else {
                "global".to_string()
            }
        }
        "global" | "project" | "merged" => scope,
        _ => "global".to_string(),
    }
}

/// The write path for experience records.
pub fn experience_path(opts: &ScopeOpts) -> PathBuf {
    if let Some(explicit) = env("FIGHORSE_EXPERIENCE_PATH") {
        return PathBuf::from(explicit);
    }
    match effective_scope(opts).as_str() {
        "project" => project_experience_path(opts.project_dir.as_deref()),
        "merged" => {
            if project_installed(opts.project_dir.as_deref()) {
                project_experience_path(opts.project_dir.as_deref())
            } else {
                global_experience_path()
            }
        }
        _ => global_experience_path(),
    }
}

/// The ordered set of read paths (deduplicated).
pub fn experience_read_paths(opts: &ScopeOpts) -> Vec<PathBuf> {
    if let Some(explicit) = env("FIGHORSE_EXPERIENCE_PATH") {
        return vec![PathBuf::from(explicit)];
    }
    let scope = effective_scope(opts);
    let project_path = project_experience_path(opts.project_dir.as_deref());
    let global_path = global_experience_path();
    let paths = match scope.as_str() {
        "project" | "merged" => vec![project_path, global_path],
        _ => vec![global_path],
    };
    // distinct, preserving order.
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

/// Store metadata for diagnostics.
pub fn store_info(opts: &ScopeOpts) -> Value {
    let env_overrides = {
        let mut m = Map::new();
        for key in [
            "FIGHORSE_HOME",
            "FIGHORSE_EXPERIENCE_PATH",
            "FIGHORSE_EXPERIENCE_SCOPE",
            "FIGHORSE_PROJECT_DIR",
        ] {
            if let Some(v) = env(key) {
                m.insert(key.to_string(), Value::String(v));
            }
        }
        if m.is_empty() {
            Value::Null
        } else {
            Value::Object(m)
        }
    };

    json!({
        "home": config::fighorse_home().to_string_lossy(),
        "scope": effective_scope(opts),
        "project_dir": resolve_project_dir(opts.project_dir.as_deref()).to_string_lossy(),
        "project_config": project_config_path(opts.project_dir.as_deref()).to_string_lossy(),
        "project_installed": project_installed(opts.project_dir.as_deref()),
        "write_path": experience_path(opts).to_string_lossy(),
        "read_paths": experience_read_paths(opts)
            .iter()
            .map(|p| Value::String(p.to_string_lossy().into_owned()))
            .collect::<Vec<_>>(),
        "env_overrides": env_overrides,
    })
}

fn blank_to_none(value: Option<&Value>) -> Option<String> {
    let v = value?;
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Null => return None,
        other => other.to_string(),
    };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_scalar(value: Option<&Value>, default: Option<&str>) -> Option<String> {
    match blank_to_none(value) {
        Some(v) => Some(v.to_lowercase()),
        None => default.map(String::from),
    }
}

/// Drop nil/blank/empty entries from a map, returning None if empty.
fn clean_map(m: Map<String, Value>) -> Option<Value> {
    let mut out = Map::new();
    for (k, v) in m {
        let keep = match &v {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            _ => true,
        };
        if keep {
            out.insert(k, v);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(Value::Object(out))
    }
}

fn normalize_tags(tags: Option<&Value>) -> Option<Value> {
    let values: Vec<String> = match tags {
        Some(Value::String(s)) => s.split(',').map(String::from).collect(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect(),
        _ => vec![],
    };
    let mut seen = std::collections::HashSet::new();
    let cleaned: Vec<Value> = values
        .into_iter()
        .filter_map(|v| {
            let t = v.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_lowercase())
            }
        })
        .filter(|v| seen.insert(v.clone()))
        .map(Value::String)
        .collect();
    if cleaned.is_empty() {
        None
    } else {
        Some(Value::Array(cleaned))
    }
}

/// The versioned schema descriptor.
pub fn schema() -> Value {
    let opts = ScopeOpts::default();
    json!({
        "kind": "fighorse.experience-schema.v1",
        "record_kind": RECORD_KIND,
        "schema_version": SCHEMA_VERSION,
        "store": {
            "format": "jsonl",
            "default_home": config::fighorse_home().to_string_lossy(),
            "global_path": global_experience_path().to_string_lossy(),
            "project_path": project_experience_path(None).to_string_lossy(),
            "write_path": experience_path(&opts).to_string_lossy(),
            "read_paths": experience_read_paths(&opts)
                .iter().map(|p| Value::String(p.to_string_lossy().into_owned())).collect::<Vec<_>>(),
            "rules": [
                "FIGHORSE_EXPERIENCE_PATH is an exact override.",
                "Global experience writes to ~/.fighorse/experience/global.jsonl by default.",
                "Run fighorse install project to enable project experience at ./.fighorse/experience.jsonl.",
                "When a project is installed, reads include project experience first and global experience second.",
                "Use FIGHORSE_EXPERIENCE_SCOPE=global|project|merged to override automatic scope."
            ],
            "append_only": true
        },
        "compatibility": {
            "rule": "New fields may be added. Readers must ignore unknown fields. Required v1 fields remain stable.",
            "required_fields": ["kind", "schema_version", "id", "created_at", "summary", "lesson"],
            "stable_fields": ["source", "target", "category", "severity", "summary", "lesson", "recommendation", "evidence", "tags", "applies_to", "tool_context"]
        },
        "fields": {
            "summary": "Short problem or insight title. Required.",
            "lesson": "Reusable lesson learned. Required.",
            "category": "layout|typography|asset-export|platform|workflow|debugging|mcp|cli|other",
            "severity": "info|warning|critical",
            "source": {"figma_url": "Optional Figma URL", "file_key": "Optional Figma file key", "node_id": "Optional Figma node id"},
            "target": {"platform": "Optional target platform/framework, e.g. android-compose, ios-swiftui, web-react, flutter", "asset_format": "Optional export format, e.g. png/svg/pdf/webp"},
            "recommendation": "Action AI should take next time.",
            "evidence": "What happened: screenshot diff, build error, overlap, etc.",
            "tags": "Comma-separated string or array.",
            "tool_context": {"client": "cursor|codex|kimi|claude|opencode|other", "command": "CLI command or MCP tool that surfaced the issue"}
        }
    })
}

fn field<'a>(input: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let obj = input.as_object()?;
    for k in keys {
        if let Some(v) = obj.get(*k) {
            return Some(v);
        }
    }
    None
}

fn now_iso() -> String {
    now_iso_public()
}

/// UTC ISO-8601 timestamp (with milliseconds), matching JS `Date.toISOString()`.
/// Exposed for reuse by the installer.
pub fn now_iso_public() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    iso8601_from_epoch(dur.as_secs(), dur.subsec_millis())
}

/// Format a UTC ISO-8601 timestamp (with milliseconds) from an epoch, matching
/// JavaScript's `Date.toISOString()` shape (e.g. `2026-07-10T01:02:03.004Z`).
fn iso8601_from_epoch(secs: u64, millis: u32) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hour, min, sec) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}.{millis:03}Z")
}

fn random_id() -> String {
    // No Date.now/random determinism concerns here (real CLI). Use time + a
    // pseudo-random suffix derived from nanos.
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}-{}", now.as_millis(), now.subsec_nanos() % 1_000_000)
}

/// Normalize a raw input record into a stored record, or error if required
/// fields are missing.
pub fn normalize_record(input: &Value) -> Result<Value> {
    let summary = blank_to_none(input.get("summary"))
        .ok_or_else(|| Error::Other("experience.summary is required".into()))?;
    let lesson = blank_to_none(input.get("lesson"))
        .ok_or_else(|| Error::Other("experience.lesson is required".into()))?;
    let timestamp = now_iso();

    let source_obj = input.get("source");
    let target_obj = input.get("target");
    let tool_obj = input.get("tool_context");

    let source = {
        let mut m = Map::new();
        m.insert(
            "figma_url".into(),
            str_or_null(
                field(input, &["figma_url", "figma-url"])
                    .or_else(|| source_obj.and_then(|s| field(s, &["figma_url", "figma-url"]))),
            ),
        );
        m.insert(
            "file_key".into(),
            str_or_null(
                field(input, &["file_key", "file-key"])
                    .or_else(|| source_obj.and_then(|s| field(s, &["file_key", "file-key"]))),
            ),
        );
        m.insert(
            "node_id".into(),
            str_or_null(
                field(input, &["node_id", "node-id"])
                    .or_else(|| source_obj.and_then(|s| field(s, &["node_id", "node-id"]))),
            ),
        );
        clean_map(m)
    };

    let target = {
        let platform = normalize_scalar(
            field(input, &["platform"])
                .or_else(|| target_obj.and_then(|t| field(t, &["platform"]))),
            None,
        );
        let asset_format = normalize_scalar(
            field(input, &["asset_format", "asset-format"])
                .or_else(|| target_obj.and_then(|t| field(t, &["asset_format", "asset-format"]))),
            None,
        );
        let mut m = Map::new();
        m.insert("platform".into(), opt_string(platform));
        m.insert("asset_format".into(), opt_string(asset_format));
        clean_map(m)
    };

    let tool_context = {
        let mut m = Map::new();
        m.insert(
            "client".into(),
            str_or_null(
                field(input, &["client"]).or_else(|| tool_obj.and_then(|t| field(t, &["client"]))),
            ),
        );
        m.insert(
            "command".into(),
            str_or_null(
                field(input, &["command"])
                    .or_else(|| tool_obj.and_then(|t| field(t, &["command"]))),
            ),
        );
        clean_map(m)
    };

    let mut record = Map::new();
    record.insert("kind".into(), Value::String(RECORD_KIND.into()));
    record.insert("schema_version".into(), json!(SCHEMA_VERSION));
    record.insert(
        "id".into(),
        Value::String(blank_to_none(input.get("id")).unwrap_or_else(random_id)),
    );
    record.insert(
        "created_at".into(),
        Value::String(blank_to_none(input.get("created_at")).unwrap_or_else(|| timestamp.clone())),
    );
    record.insert(
        "updated_at".into(),
        Value::String(blank_to_none(input.get("updated_at")).unwrap_or_else(|| timestamp.clone())),
    );
    record.insert(
        "category".into(),
        Value::String(normalize_scalar(input.get("category"), Some("workflow")).unwrap()),
    );
    record.insert(
        "severity".into(),
        Value::String(normalize_scalar(input.get("severity"), Some("info")).unwrap()),
    );
    if let Some(s) = source {
        record.insert("source".into(), s);
    }
    if let Some(t) = target {
        record.insert("target".into(), t);
    }
    record.insert("summary".into(), Value::String(summary));
    record.insert("lesson".into(), Value::String(lesson));
    if let Some(r) = blank_to_none(input.get("recommendation")) {
        record.insert("recommendation".into(), Value::String(r));
    }
    if let Some(e) = blank_to_none(input.get("evidence")) {
        record.insert("evidence".into(), Value::String(e));
    }
    if let Some(tags) = normalize_tags(input.get("tags")) {
        record.insert("tags".into(), tags);
    }
    if let Some(a) = input.get("applies_to") {
        if !a.is_null() {
            record.insert("applies_to".into(), a.clone());
        }
    }
    if let Some(tc) = tool_context {
        record.insert("tool_context".into(), tc);
    }

    // clean_map over the whole record (drops any nil/blank/empty just added).
    Ok(clean_map(record).unwrap_or(Value::Object(Default::default())))
}

fn str_or_null(v: Option<&Value>) -> Value {
    match blank_to_none(v) {
        Some(s) => Value::String(s),
        None => Value::Null,
    }
}

fn opt_string(v: Option<String>) -> Value {
    match v {
        Some(s) => Value::String(s),
        None => Value::Null,
    }
}

/// Append a normalized record to the store.
pub fn add(input: &Value, opts: &ScopeOpts) -> Result<Value> {
    let record = normalize_record(input)?;
    let store = experience_path(opts);
    if let Some(parent) = store.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = format!("{}\n", serde_json::to_string(&record)?);
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&store)?;
    file.write_all(line.as_bytes())?;

    Ok(json!({
        "kind": "fighorse.experience-write.v1",
        "store_path": store.to_string_lossy(),
        "store": store_info(opts),
        "record": record,
        "next_step": "Call list_experiences or fighorse experience summary before the next Figma replication task."
    }))
}

fn parse_line(line: &str) -> Option<Value> {
    if line.trim().is_empty() {
        return None;
    }
    serde_json::from_str(line).ok()
}

fn read_store(store: &PathBuf) -> Vec<Value> {
    let content = match std::fs::read_to_string(store) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    content
        .split('\n')
        .flat_map(|line| line.strip_suffix('\r').or(Some(line)))
        .filter_map(parse_line)
        .filter_map(|rec| normalize_record(&rec).ok())
        .collect()
}

/// Read all records across the resolved read paths (newest first per store).
pub fn read_all(opts: &ScopeOpts) -> Vec<Value> {
    let mut out = Vec::new();
    for path in experience_read_paths(opts) {
        let mut records = read_store(&path);
        records.reverse();
        out.extend(records);
    }
    out
}

fn matches_text(needle: Option<&str>, haystack: Option<&Value>) -> bool {
    let needle = normalize_scalar(needle.map(|s| Value::String(s.to_string())).as_ref(), None);
    let haystack = normalize_scalar(haystack, None);
    match (needle, haystack) {
        (None, _) => true,
        (_, None) => true,
        (_, Some(h)) if h == "unspecified" => true,
        (Some(n), Some(h)) => n == h || h.contains(&n) || n.contains(&h),
    }
}

/// Filters for experience listing.
#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub platform: Option<String>,
    pub asset_format: Option<String>,
    pub category: Option<String>,
    pub tag: Option<String>,
}

fn matches(record: &Value, f: &Filters) -> bool {
    let target = record.get("target");
    matches_text(
        f.platform.as_deref(),
        target.and_then(|t| t.get("platform")),
    ) && matches_text(
        f.asset_format.as_deref(),
        target.and_then(|t| t.get("asset_format")),
    ) && matches_text(f.category.as_deref(), record.get("category"))
        && match f.tag.as_deref().and_then(|t| {
            let t = t.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_lowercase())
            }
        }) {
            None => true,
            Some(tag) => record
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(tag.as_str())))
                .unwrap_or(false),
        }
}

fn compact_record(record: &Value) -> Value {
    let keys = [
        "id",
        "created_at",
        "category",
        "severity",
        "source",
        "target",
        "summary",
        "lesson",
        "recommendation",
        "evidence",
        "tags",
        "tool_context",
    ];
    let mut m = Map::new();
    if let Some(obj) = record.as_object() {
        for k in keys {
            if let Some(v) = obj.get(k) {
                m.insert(k.to_string(), v.clone());
            }
        }
    }
    Value::Object(m)
}

/// List matching experiences (prompt-ready summary).
pub fn list_experiences(filters: &Filters, limit: usize, opts: &ScopeOpts) -> Value {
    let records = read_all(opts);
    let total = records.len();
    let filtered: Vec<Value> = records
        .iter()
        .filter(|r| matches(r, filters))
        .take(limit)
        .map(compact_record)
        .collect();

    let filters_map = {
        let mut m = Map::new();
        m.insert("platform".into(), opt_string(filters.platform.clone()));
        m.insert(
            "asset_format".into(),
            opt_string(filters.asset_format.clone()),
        );
        m.insert("category".into(), opt_string(filters.category.clone()));
        m.insert("tag".into(), opt_string(filters.tag.clone()));
        m.insert("scope".into(), opt_string(opts.scope.clone()));
        m.insert("project_dir".into(), opt_string(opts.project_dir.clone()));
        m.insert("limit".into(), json!(limit));
        clean_map(m).unwrap_or(Value::Object(Default::default()))
    };

    json!({
        "kind": SUMMARY_KIND,
        "schema_version": SCHEMA_VERSION,
        "store_path": experience_path(opts).to_string_lossy(),
        "store": store_info(opts),
        "total_count": total,
        "returned_count": filtered.len(),
        "filters": filters_map,
        "records": filtered,
    })
}

/// Prompt-ready guidance wrapping a filtered experience summary.
pub fn guidance(filters: &Filters, limit: usize, opts: &ScopeOpts) -> Value {
    let summary = list_experiences(filters, limit, opts);
    json!({
        "kind": GUIDANCE_KIND,
        "schema_version": SCHEMA_VERSION,
        "instruction": "Before implementation or after a mismatch, review relevant local experiences. After discovering a new reusable lesson, call record_experience or fighorse experience add.",
        "ai_contract": guidance::ai_contract(),
        "output_locations": guidance::output_location_guidance(),
        "record_when": [
            "A Figma-to-code mismatch is fixed.",
            "A platform-specific rule is learned.",
            "An export format or asset pipeline issue is discovered.",
            "A prompt/workflow step prevents a repeated error.",
            "A real app screenshot reveals overlap, clipping, wrong typography, or wrong system chrome handling."
        ],
        "schema": schema(),
        "summary": summary,
    })
}

/// Markdown rendering of guidance output.
pub fn guidance_markdown(data: &Value) -> String {
    let records = data
        .get("summary")
        .and_then(|s| s.get("records"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let instruction = data
        .get("instruction")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let store_path = data
        .get("summary")
        .and_then(|s| s.get("store_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let record_when: Vec<String> = data
        .get("record_when")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| format!("- {}", v.as_str().unwrap_or("")))
                .collect()
        })
        .unwrap_or_default();

    let records_md = if records.is_empty() {
        "No matching local experience yet. Record lessons after this task.".to_string()
    } else {
        records
            .iter()
            .map(|r| {
                let summary = r.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                let lesson = r.get("lesson").and_then(|v| v.as_str()).unwrap_or("");
                let mut s = format!("- **{summary}**\n  Lesson: {lesson}");
                if let Some(rec) = r.get("recommendation").and_then(|v| v.as_str()) {
                    s.push_str(&format!("\n  Recommendation: {rec}"));
                }
                s
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        "# fighorse Learned Experience\n\n{instruction}\n\nStore: `{store_path}`\n\n## When To Record\n\n{}\n\n## Relevant Records\n\n{}",
        record_when.join("\n"),
        records_md
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_store<F: FnOnce(&PathBuf)>(f: F) {
        // Serialize: these tests mutate the process-global FIGHORSE_EXPERIENCE_PATH.
        use std::sync::Mutex;
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let dir = std::env::temp_dir().join(format!("fighorse-exp-{}", random_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = dir.join("experience.jsonl");
        std::env::set_var("FIGHORSE_EXPERIENCE_PATH", &store);
        f(&store);
        std::env::remove_var("FIGHORSE_EXPERIENCE_PATH");
    }

    #[test]
    fn add_list_and_summarize() {
        with_temp_store(|store| {
            let input = json!({
                "summary": "Compose rows overlapped",
                "lesson": "Use Column or LazyColumn for repeated rows; Box stacks children.",
                "category": "layout",
                "severity": "warning",
                "platform": "android-compose",
                "asset_format": "png",
                "tags": "compose,overlap",
                "evidence": "Real device screenshot showed duplicated text.",
                "recommendation": "Inspect repeated children before implementing a list.",
                "client": "codex",
                "command": "record_experience"
            });
            let opts = ScopeOpts::default();
            let write = add(&input, &opts).unwrap();
            assert_eq!(write["kind"], "fighorse.experience-write.v1");
            assert_eq!(write["store_path"], store.to_string_lossy().as_ref());
            assert_eq!(write["record"]["kind"], RECORD_KIND);
            assert_eq!(write["record"]["schema_version"], 1);
            assert_eq!(write["record"]["target"]["platform"], "android-compose");
            assert!(store.exists());

            let filters = Filters {
                platform: Some("android-compose".into()),
                asset_format: Some("png".into()),
                tag: Some("compose".into()),
                ..Default::default()
            };
            let listed = list_experiences(&filters, 4, &opts);
            assert_eq!(listed["kind"], "fighorse.experience-summary.v1");
            assert_eq!(listed["total_count"], 1);
            assert_eq!(listed["returned_count"], 1);
            assert_eq!(listed["records"][0]["summary"], "Compose rows overlapped");

            let g = guidance(
                &Filters {
                    platform: Some("android-compose".into()),
                    asset_format: Some("png".into()),
                    ..Default::default()
                },
                6,
                &opts,
            );
            assert_eq!(g["kind"], "fighorse.learned-guidance.v1");
            assert_eq!(g["schema"]["kind"], "fighorse.experience-schema.v1");
            assert_eq!(g["ai_contract"]["kind"], "fighorse.ai-contract.v1");
            let md = guidance_markdown(&g);
            assert!(md.contains("Compose rows overlapped"));
        });
    }

    #[test]
    fn global_records_apply_without_target_filters() {
        with_temp_store(|_store| {
            let opts = ScopeOpts::default();
            add(
                &json!({
                    "summary": "Always compare screenshots",
                    "lesson": "Run the app and compare against the Figma screenshot before finalizing."
                }),
                &opts,
            )
            .unwrap();
            let listed = list_experiences(
                &Filters {
                    platform: Some("ios-swiftui".into()),
                    asset_format: Some("pdf".into()),
                    ..Default::default()
                },
                8,
                &opts,
            );
            assert_eq!(listed["returned_count"], 1);
            assert_eq!(listed["records"][0]["category"], "workflow");
        });
    }

    #[test]
    fn iso8601_format() {
        // 2021-01-01T00:00:00.000Z is epoch 1609459200.
        assert_eq!(
            iso8601_from_epoch(1_609_459_200, 0),
            "2021-01-01T00:00:00.000Z"
        );
    }
}
