use crate::code_connect::model::{CodeConnectDocument, ValidationReport, COMPATIBILITY_COMMIT};
use crate::error::{Error, Result};
use crate::http;
use crate::url as figma_url;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn protocol_incompatible(phase: &str, detail: &str) -> Error {
    Error::Other(format!(
        "protocol_incompatible: Code Connect {phase} response did not match baseline {COMPATIBILITY_COMMIT}: {detail}"
    ))
}

fn parse_figma_node_url(figma_node: &str) -> Result<(String, String)> {
    let parsed = url::Url::parse(figma_node)
        .map_err(|e| Error::Usage(format!("Invalid Figma node URL `{figma_node}`: {e}")))?;
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.collect())
        .unwrap_or_default();
    let file_key = segments
        .windows(2)
        .find_map(|w| {
            if matches!(w[0], "file" | "design") {
                Some(w[1].to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| Error::Usage(format!("Invalid Figma node URL `{figma_node}`")))?;
    let node_id = parsed
        .query_pairs()
        .find_map(|(k, v)| {
            if k == "node-id" {
                Some(figma_url::normalize_node_id(&v))
            } else {
                None
            }
        })
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
            let data = http::get(&path, Some(token), &[("ids", Value::String(joined))]).await?;
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

pub async fn preview_documents(
    token: &str,
    docs: &[CodeConnectDocument],
    render_combinations: Option<Value>,
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
            let result = http::post(
                path,
                Some(token),
                &[("file_key", Value::String(file_key.clone()))],
                Some(&body),
            )
            .await?;
            all_results.push(assert_meta("preview", result)?);
        }
    }
    if all_results.len() == 1 {
        Ok(all_results.remove(0))
    } else {
        Ok(json!({"meta": {"results": all_results}}))
    }
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
    let value = http::post("/v1/code_connect", Some(token), &params, Some(&payload)).await?;
    assert_meta("publish", value)
}

pub async fn unpublish_documents(token: &str, nodes: &[(&str, &str)]) -> Result<Value> {
    let body = json!({
        "nodes_to_delete": nodes.iter().map(|(figma_node, label)| {
            json!({"figmaNode": figma_node, "label": label})
        }).collect::<Vec<_>>()
    });
    let value = http::request("DELETE", "/v1/code_connect", Some(token), &[], Some(&body)).await?;
    assert_meta("unpublish", value)
}
