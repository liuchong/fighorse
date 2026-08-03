use crate::code_connect::model::{
    CodeConnectDocument, CodeConnectProject, CodeConnectProjectConfig, TemplateData,
    COMPATIBILITY_CLI_VERSION,
};
use crate::error::{Error, Result};
use serde_json::{json, Value};
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

fn convert_figma_import(template: String) -> String {
    template
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed == "import figma from 'figma'"
                || trimmed == "import figma from 'figma';"
                || trimmed == "import figma from \"figma\""
                || trimmed == "import figma from \"figma\";"
            {
                "const figma = require('figma')".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

fn parse_batch_file(path: &Path, project: &CodeConnectProject) -> Result<Vec<CodeConnectDocument>> {
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

pub fn parse_project(project: &CodeConnectProject) -> Result<Vec<CodeConnectDocument>> {
    let mut docs = Vec::new();
    for path in &project.files {
        if path
            .file_name()
            .and_then(|v| v.to_str())
            .map(|name| name.ends_with(".figma.batch.json"))
            .unwrap_or(false)
        {
            docs.extend(parse_batch_file(path, project)?);
        } else if let Some(doc) = parse_raw_file(path, project)? {
            docs.push(doc);
        }
    }
    Ok(docs)
}
