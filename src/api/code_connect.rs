use crate::code_connect::model::{
    CODE_CONNECT_USER_AGENT, COMPATIBILITY_COMMIT, CodeConnectDocument, ValidationReport,
};
use crate::error::{Error, Result};
use crate::http;
use crate::url as figma_url;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

fn protocol_incompatible(phase: &str, detail: &str) -> Error {
    Error::Other(format!(
        "protocol_incompatible: Code Connect {phase} response did not match baseline {COMPATIBILITY_COMMIT}: {detail}"
    ))
}

fn cc_headers() -> [(&'static str, &'static str); 1] {
    [("User-Agent", CODE_CONNECT_USER_AGENT)]
}

fn parse_figma_node_url(figma_node: &str) -> Result<(String, String)> {
    let parsed = figma_url::parse_figma_url(figma_node);
    if !parsed.valid {
        return Err(Error::Usage(format!(
            "Invalid Figma node URL `{figma_node}`: {}",
            parsed.error.as_deref().unwrap_or("missing file key")
        )));
    }
    let file_key = parsed
        .file_key
        .ok_or_else(|| Error::Usage(format!("Invalid Figma node URL `{figma_node}`")))?;
    let node_id = parsed
        .node_id
        .ok_or_else(|| Error::Usage(format!("Figma node URL missing node-id: {figma_node}")))?;
    Ok((file_key, node_id))
}

fn node_groups(docs: &[CodeConnectDocument]) -> Result<BTreeMap<String, Vec<String>>> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for doc in docs {
        let (file_key, node_id) = parse_figma_node_url(&doc.figma_node)?;
        let ids = grouped.entry(file_key).or_default();
        if !ids.contains(&node_id) {
            ids.push(node_id);
        }
    }
    Ok(grouped)
}

pub async fn validate_documents(
    token: &str,
    docs: &[CodeConnectDocument],
) -> Result<ValidationReport> {
    let mut report = ValidationReport {
        valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    for (file_key, ids) in node_groups(docs)? {
        for chunk in ids.chunks(400) {
            let joined = chunk.join(",");
            let path = format!("/v1/files/{}/nodes", http::path_segment(&file_key));
            let data = http::request_with_headers(
                "GET",
                &path,
                Some(token),
                &[("ids", Value::String(joined))],
                None,
                &cc_headers(),
            )
            .await?;
            let nodes = data
                .get("nodes")
                .and_then(|v| v.as_object())
                .ok_or_else(|| protocol_incompatible("validate", "missing nodes object"))?;
            for id in chunk {
                let node = nodes.get(id).ok_or_else(|| {
                    Error::Usage(format!(
                        "Validation failed: node {id} not found in {file_key}"
                    ))
                })?;
                let doc = node.get("document").ok_or_else(|| {
                    Error::Usage(format!("Validation failed: node {id} missing document"))
                })?;
                let node_type = doc.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if !matches!(node_type, "COMPONENT" | "COMPONENT_SET") {
                    report.valid = false;
                    report.errors.push(format!(
                        "Validation failed for {file_key}/{id}: node is not a component or component set"
                    ));
                }
                if node
                    .get("components")
                    .and_then(|c| c.get(id))
                    .and_then(|c| c.get("componentSetId"))
                    .is_some()
                {
                    report.valid = false;
                    report.errors.push(format!(
                        "Validation failed for {file_key}/{id}: node is a variant, not a top-level component"
                    ));
                }
            }
        }
    }
    Ok(report)
}

fn docs_payload(docs: &[CodeConnectDocument]) -> Value {
    Value::Array(docs.iter().map(CodeConnectDocument::upload_value).collect())
}

fn assert_meta(phase: &str, value: Value) -> Result<Value> {
    if value.get("meta").is_none() {
        return Err(protocol_incompatible(phase, "missing meta object"));
    }
    Ok(value)
}

fn deduplicate_preview_array(items: &[Value]) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let key = format!(
            "{}|{}|{}|{}",
            item.get("nodeId").and_then(Value::as_str).unwrap_or(""),
            item.get("label").and_then(Value::as_str).unwrap_or(""),
            item.get("language").and_then(Value::as_str).unwrap_or(""),
            item.get("snippet")
                .or_else(|| item.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("")
        );
        if seen.insert(key) {
            out.push(item.clone());
        }
    }
    out
}

fn deduplicate_preview_value(value: &mut Value) {
    if let Some(results) = value
        .get_mut("meta")
        .and_then(|m| m.get_mut("results"))
        .and_then(Value::as_array_mut)
    {
        // Either flat results or nested chunk responses.
        if results.iter().any(|r| r.get("meta").is_some()) {
            for sub in results.iter_mut() {
                deduplicate_preview_value(sub);
            }
        } else {
            let deduped = deduplicate_preview_array(results);
            *results = deduped;
        }
    }
}

pub async fn preview_documents(
    token: &str,
    docs: &[CodeConnectDocument],
    render_combinations: Option<Value>,
) -> Result<Value> {
    preview_documents_with_options(token, docs, render_combinations, false).await
}

pub async fn preview_documents_with_options(
    token: &str,
    docs: &[CodeConnectDocument],
    render_combinations: Option<Value>,
    unique: bool,
) -> Result<Value> {
    let mut all_results = Vec::new();
    for (file_key, ids) in node_groups(docs)? {
        for chunk in ids.chunks(50) {
            let path = "/v1/code_connect/preview_snippets";
            let mut body = json!({
                "nodeIds": chunk,
                "figmaDocs": {"all": docs_payload(docs)}
            });
            if let Some(combinations) = &render_combinations {
                body["renderCombinations"] = combinations.clone();
            }
            let result = http::request_with_headers(
                "POST",
                path,
                Some(token),
                &[("file_key", Value::String(file_key.clone()))],
                Some(&body),
                &cc_headers(),
            )
            .await?;
            all_results.push(assert_meta("preview", result)?);
        }
    }
    let mut combined = if all_results.len() == 1 {
        all_results.remove(0)
    } else {
        json!({"meta": {"results": all_results}})
    };
    if unique {
        deduplicate_preview_value(&mut combined);
    }
    Ok(combined)
}

fn enrich_publish_diagnostics(mut value: Value) -> Value {
    let mut pro_restrictions = Vec::new();
    if let Some(failed_nodes) = value
        .get("meta")
        .and_then(|m| m.get("failed_nodes"))
        .and_then(Value::as_array)
    {
        for failed in failed_nodes {
            let node_url = failed
                .get("figmaNode")
                .or_else(|| failed.get("figma_node"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if let Some(errors) = failed.get("errors").and_then(Value::as_array) {
                for err in errors {
                    let code = err.get("code").and_then(Value::as_str).unwrap_or("");
                    match code {
                        "CODE_CONNECT_PRO_LIBRARY_INACTIVE" => {
                            pro_restrictions.push(json!({
                                "code": code,
                                "figma_node": node_url,
                                "message": "Professional plan allows only one active Code Connect library. Activate this library in Figma Dev Mode settings before publishing."
                            }));
                        }
                        "CODE_CONNECT_PRO_FRAMEWORK_INACTIVE" => {
                            let active_fw = err
                                .get("active_framework")
                                .or_else(|| err.get("activeFramework"))
                                .cloned()
                                .unwrap_or(Value::Null);
                            pro_restrictions.push(json!({
                                "code": code,
                                "figma_node": node_url,
                                "active_framework": active_fw,
                                "message": "Professional plan allows only one active Code Connect framework label per library."
                            }));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    let new_active = value
        .get("meta")
        .and_then(|m| m.get("new_active_selections"))
        .and_then(Value::as_array)
        .filter(|arr| !arr.is_empty())
        .cloned();

    if (!pro_restrictions.is_empty() || new_active.is_some())
        && let Some(obj) = value.as_object_mut()
    {
        obj.insert(
            "diagnostics".into(),
            json!({
                "pro_plan_restrictions": pro_restrictions,
                "new_active_selections": new_active.unwrap_or_default(),
            }),
        );
    }
    value
}

pub async fn publish_documents(
    token: &str,
    docs: &[CodeConnectDocument],
    force: bool,
    batch_size: Option<usize>,
) -> Result<Value> {
    let params = if force {
        vec![("force", Value::String("true".into()))]
    } else {
        vec![]
    };
    let payload = docs_payload(docs);
    let size = serde_json::to_vec(&payload)?.len();
    if size > 5 * 1024 * 1024 {
        return Err(Error::Usage(
            "Code Connect publish payload exceeds the 5 MiB request limit".into(),
        ));
    }
    if batch_size.is_some() {
        // The public function accepts the option so callers can preserve CLI
        // shape; grouping support can be expanded without changing the API.
    }
    let value = http::request_with_headers(
        "POST",
        "/v1/code_connect",
        Some(token),
        &params,
        Some(&payload),
        &cc_headers(),
    )
    .await?;
    let checked = assert_meta("publish", value)?;
    Ok(enrich_publish_diagnostics(checked))
}

pub async fn unpublish_documents(token: &str, nodes: &[(&str, &str)]) -> Result<Value> {
    let body = json!({
        "nodes_to_delete": nodes.iter().map(|(figma_node, label)| {
            json!({"figmaNode": figma_node, "label": label})
        }).collect::<Vec<_>>()
    });
    let value = http::request_with_headers(
        "DELETE",
        "/v1/code_connect",
        Some(token),
        &[],
        Some(&body),
        &cc_headers(),
    )
    .await?;
    assert_meta("unpublish", value)
}
