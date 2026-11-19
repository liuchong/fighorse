use crate::code_connect::model::{
    COMPATIBILITY_CLI_VERSION, CodeConnectDocument, CodeConnectProject, CodeConnectProjectConfig,
    TemplateData,
};
use crate::error::{Error, Result};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_TEMPLATE_SIZE: usize = 1024 * 1024;

#[derive(Default)]
struct MetadataFields {
    url: Option<String>,
    component: Option<String>,
    source: Option<String>,
}

fn is_metadata_directive(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("// url=")
        || trimmed.starts_with("// component=")
        || trimmed.starts_with("// source=")
}

fn extract_metadata(content: &str) -> (MetadataFields, usize) {
    let mut fields = MetadataFields::default();
    let mut template_start_line = 0;
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            if let Some(rest) = trimmed.strip_prefix("//") {
                let rest = rest.trim();
                if let Some(value) = rest.strip_prefix("url=") {
                    fields.url = Some(value.trim().to_string());
                    template_start_line = idx + 1;
                } else if let Some(value) = rest.strip_prefix("component=") {
                    fields.component = Some(value.trim().to_string());
                    template_start_line = idx + 1;
                } else if let Some(value) = rest.strip_prefix("source=") {
                    fields.source = Some(value.trim().to_string());
                    template_start_line = idx + 1;
                }
            }
            continue;
        }
        break;
    }
    (fields, template_start_line)
}

fn has_raw_directive(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            if is_metadata_directive(trimmed) {
                return true;
            }
            continue;
        }
        break;
    }
    false
}

fn apply_substitutions(mut url: String, config: &CodeConnectProjectConfig) -> String {
    for (from, to) in &config.document_url_substitutions {
        url = url.replace(from, to);
    }
    url
}

fn strip_leading_metadata(content: &str, template_start_line: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut start = template_start_line.min(lines.len());
    while start < lines.len() {
        let trimmed = lines[start].trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            start += 1;
        } else {
            break;
        }
    }
    lines[start..].join("\n")
}

/// Rewrite `import figma from 'figma'` (including optional `;` and trailing `//` comment)
/// to `const figma = require('figma')`, matching `@figma/code-connect` v2.1.0.
fn convert_figma_import(template: String) -> String {
    let import_re = regex::Regex::new(
        r#"^[ \t]*import[ \t]+figma[ \t]+from[ \t]+['"]figma['"][ \t]*;?[ \t]*(?://[^\r\n]*)?$"#,
    )
    .expect("valid figma import regex");
    template
        .lines()
        .map(|line| {
            if import_re.is_match(line) {
                "const figma = require('figma')".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_legacy_parser_file(content: &str) -> bool {
    content.contains("figma.connect(")
        || content.contains("@figma/code-connect")
        || content.contains("FigmaConnectMeta")
}

fn is_shared_template_filename(path: &Path) -> bool {
    path.file_name()
        .and_then(|v| v.to_str())
        .is_some_and(|name| {
            name.ends_with(".figma.template.ts") || name.ends_with(".figma.template.js")
        })
}

fn parse_raw_file(
    path: &Path,
    project: &CodeConnectProject,
) -> Result<Option<CodeConnectDocument>> {
    let content = fs::read_to_string(path)?;
    if !has_raw_directive(&content) {
        if content.contains("codeProperties") {
            return Ok(None);
        }
        if is_legacy_parser_file(&content) {
            return Err(Error::Usage(format!(
                "Legacy parser-based Code Connect file is not supported in Code Connect v2.x: {}. Migrate from `figma.connect(...)` / `@figma/code-connect` to a parserless template with a leading `// url=<figma-node-url>` directive and `export default {{ example: figma.code`...` }}`.",
                path.display()
            )));
        }
        return Err(Error::Usage(format!(
            "Missing required // url= metadata in {}",
            path.display()
        )));
    }
    let (fields, template_start_line) = extract_metadata(&content);
    let figma_node = fields.url.ok_or_else(|| {
        Error::Usage(format!(
            "Missing required // url= metadata in {}",
            path.display()
        ))
    })?;
    let template = convert_figma_import(strip_leading_metadata(&content, template_start_line));
    if template.len() > MAX_TEMPLATE_SIZE {
        return Err(Error::Usage(format!(
            "Template {} exceeds the 1 MiB Code Connect limit",
            path.display()
        )));
    }
    let label = project
        .config
        .label
        .clone()
        .unwrap_or_else(|| "Code".to_string());
    let language = project
        .config
        .language
        .clone()
        .unwrap_or_else(|| "plaintext".to_string());
    Ok(Some(CodeConnectDocument {
        figma_node: apply_substitutions(figma_node, &project.config),
        component: fields.component,
        variant: None,
        template,
        template_data: TemplateData {
            props: None,
            imports: None,
            nestable: Some(true),
            is_parserless: Some(true),
        },
        language,
        label,
        links: None,
        source: fields.source,
        source_location: Some(json!({"line": -1})),
        metadata: json!({"cliVersion": COMPATIBILITY_CLI_VERSION}),
        code_connect_file_path: Some(path.to_string_lossy().to_string()),
    }))
}

fn as_entries(value: Value) -> Vec<Value> {
    match value {
        Value::Array(arr) => arr,
        other => vec![other],
    }
}

fn parse_batch_file_filtered(
    path: &Path,
    project: &CodeConnectProject,
    only_template: Option<&Path>,
    referenced_templates: &mut HashSet<PathBuf>,
) -> Result<Vec<CodeConnectDocument>> {
    let raw = fs::read_to_string(path)?;
    let root: Value = serde_json::from_str(&raw)?;
    let mut docs = Vec::new();
    for group in as_entries(root) {
        let template_file = group
            .get("templateFile")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Usage(format!("Missing templateFile in {}", path.display())))?;
        let components = group
            .get("components")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Usage(format!("Missing components in {}", path.display())))?;
        let template_path = resolve_under_root(&project.config.root, template_file)?;
        referenced_templates.insert(template_path.clone());
        if let Some(filter_target) = only_template
            && template_path != filter_target
        {
            continue;
        }
        let template_content = fs::read_to_string(&template_path)?;
        let (_, template_start_line) = extract_metadata(&template_content);
        let template = convert_figma_import(strip_leading_metadata(
            &template_content,
            template_start_line,
        ));
        for component in components {
            let figma_node = component
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Usage(format!("Missing url in {}", path.display())))?;
            docs.push(CodeConnectDocument {
                figma_node: apply_substitutions(figma_node.to_string(), &project.config),
                component: component
                    .get("component")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                variant: component.get("variant").cloned(),
                template: format!(
                    "globalThis['__FIGMA_BATCH'] = {}\n{}",
                    serde_json::to_string(component)?,
                    template
                ),
                template_data: TemplateData {
                    props: None,
                    imports: None,
                    nestable: Some(true),
                    is_parserless: Some(true),
                },
                language: project
                    .config
                    .language
                    .clone()
                    .unwrap_or_else(|| "plaintext".to_string()),
                label: project
                    .config
                    .label
                    .clone()
                    .unwrap_or_else(|| "Code".to_string()),
                links: None,
                source: component
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                source_location: Some(json!({"line": -1})),
                metadata: json!({"cliVersion": COMPATIBILITY_CLI_VERSION}),
                code_connect_file_path: Some(path.to_string_lossy().to_string()),
            });
        }
    }
    Ok(docs)
}

fn resolve_under_root(root: &Path, rel: &str) -> Result<PathBuf> {
    let candidate = root.join(rel);
    let canonical = candidate.canonicalize().unwrap_or(candidate);
    if !canonical.starts_with(root) {
        return Err(Error::Usage(format!(
            "Template helper path escapes project root: {rel}"
        )));
    }
    Ok(canonical)
}

fn discover_batch_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "node_modules" || n == ".git")
            {
                continue;
            }
            discover_batch_files(&path, out)?;
        } else if meta.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".figma.batch.json"))
        {
            out.push(path);
        }
    }
    Ok(())
}

pub fn parse_project(project: &CodeConnectProject) -> Result<Vec<CodeConnectDocument>> {
    let mut docs = Vec::new();
    let mut referenced_templates = HashSet::new();

    // First pass: parse all `.figma.batch.json` files and record referenced template files.
    for path in &project.files {
        if path
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|name| name.ends_with(".figma.batch.json"))
        {
            docs.extend(parse_batch_file_filtered(
                path,
                project,
                None,
                &mut referenced_templates,
            )?);
        }
    }

    // Second pass: parse standalone `.figma.{ts,js}` files; if a single `.figma.template.{ts,js}`
    // without `// url=` was requested via `--file`, resolve its `.figma.batch.json` entries.
    for path in &project.files {
        if path
            .file_name()
            .and_then(|v| v.to_str())
            .is_some_and(|name| name.ends_with(".figma.batch.json"))
        {
            continue;
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if referenced_templates.contains(&canonical) {
            continue;
        }
        if is_shared_template_filename(path) {
            let content = fs::read_to_string(path)?;
            if !has_raw_directive(&content) {
                if project.files.len() == 1 {
                    let mut batch_files = Vec::new();
                    discover_batch_files(&project.config.root, &mut batch_files)?;
                    batch_files.sort();
                    let mut matched = Vec::new();
                    let mut dummy_refs = HashSet::new();
                    for batch in batch_files {
                        matched.extend(parse_batch_file_filtered(
                            &batch,
                            project,
                            Some(&canonical),
                            &mut dummy_refs,
                        )?);
                    }
                    if !matched.is_empty() {
                        docs.extend(matched);
                        continue;
                    }
                } else {
                    continue;
                }
            }
        }
        if let Some(doc) = parse_raw_file(path, project)? {
            docs.push(doc);
        }
    }
    Ok(docs)
}
