use crate::code_connect::model::{CodeConnectProject, CodeConnectProjectConfig};
use crate::error::{Error, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_INCLUDE: &[&str] = &[
    "**/*.figma.ts",
    "**/*.figma.js",
    "**/*.figma.template.ts",
    "**/*.figma.template.js",
    "**/*.figma.batch.json",
];

const DEFAULT_EXCLUDE: &[&str] = &["node_modules/**"];

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect()
    })
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
}

fn parse_config(root: &Path, config_path: Option<&Path>) -> Result<CodeConnectProjectConfig> {
    let path = config_path
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("figma.config.json"));
    let mut label = None;
    let mut language = None;
    let mut include: Vec<String> = DEFAULT_INCLUDE.iter().map(|s| s.to_string()).collect();
    let mut exclude: Vec<String> = DEFAULT_EXCLUDE.iter().map(|s| s.to_string()).collect();
    let mut substitutions = Vec::new();

    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let value: Value = serde_json::from_str(&raw)?;
        let cfg = value
            .get("codeConnect")
            .and_then(|v| v.as_object())
            .ok_or_else(|| Error::Usage(format!("No codeConnect object in {}", path.display())))?;

        if cfg.get("apiUrl").is_some() {
            return Err(Error::Usage(
                "figma.config.json codeConnect.apiUrl is not supported by fighorse because it can redirect Figma tokens".into(),
            ));
        }

        label = optional_string(cfg.get("label"));
        language = optional_string(cfg.get("language"));
        if let Some(custom) = string_array(cfg.get("include")) {
            include = custom;
        }
        if let Some(custom) = string_array(cfg.get("exclude")) {
            exclude.extend(custom);
        }
        if let Some(obj) = cfg
            .get("documentUrlSubstitutions")
            .and_then(|v| v.as_object())
        {
            substitutions.extend(
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|to| (k.clone(), to.to_string()))),
            );
        }
    }

    Ok(CodeConnectProjectConfig {
        root: root.to_path_buf(),
        label,
        language,
        include,
        exclude,
        document_url_substitutions: substitutions,
    })
}

fn path_contains_node_modules(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_string_lossy() == "node_modules")
}

fn matches_pattern(rel: &str, pattern: &str) -> bool {
    let rel_parts: Vec<&str> = rel.split('/').filter(|part| !part.is_empty()).collect();
    let pattern_parts: Vec<&str> = pattern.split('/').filter(|part| !part.is_empty()).collect();
    match_segments(&pattern_parts, &rel_parts)
}

fn match_segments(pattern: &[&str], rel: &[&str]) -> bool {
    match (pattern.split_first(), rel.split_first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some((segment, rest)), _) if *segment == "**" => {
            match_segments(rest, rel)
                || rel
                    .split_first()
                    .map(|(_, rel_rest)| match_segments(pattern, rel_rest))
                    .unwrap_or(false)
        }
        (Some((segment, rest_pattern)), Some((rel_segment, rest_rel))) => {
            match_segment(segment, rel_segment) && match_segments(rest_pattern, rest_rel)
        }
        (Some(_), None) => false,
    }
}

fn match_segment(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut cursor = 0;
    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if idx == 0 && !value.starts_with(part) {
            return false;
        }
        match value[cursor..].find(part) {
            Some(offset) => cursor += offset + part.len(),
            None => return false,
        }
    }
    if let Some(last) = parts.last() {
        if !last.is_empty() && !pattern.ends_with('*') && !value.ends_with(last) {
            return false;
        }
    }
    true
}

fn included(rel: &str, config: &CodeConnectProjectConfig) -> bool {
    config.include.iter().any(|p| matches_pattern(rel, p))
        && !config.exclude.iter().any(|p| matches_pattern(rel, p))
}

fn is_raw_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| matches!(ext, "ts" | "js" | "json"))
        .unwrap_or(false)
}

fn walk(
    root: &Path,
    dir: &Path,
    config: &CodeConnectProjectConfig,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if path_contains_node_modules(&path) {
                continue;
            }
            walk(root, &path, config, out)?;
        } else if meta.is_file() && is_raw_candidate(&path) {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if included(&rel, config) {
                out.push(path);
            }
        }
    }
    Ok(())
}

pub fn load_project(root: &Path, config_path: Option<&Path>) -> Result<CodeConnectProject> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let config = parse_config(&root, config_path)?;
    let mut files = Vec::new();
    walk(&root, &root, &config, &mut files)?;
    files.sort();
    if files.len() > 10_000 {
        return Err(Error::Usage(
            "Code Connect file discovery matched more than 10000 files; narrow include/exclude globs".into(),
        ));
    }
    Ok(CodeConnectProject { config, files })
}
