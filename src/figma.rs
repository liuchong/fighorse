//! Shared helpers for normalizing Figma REST responses.
//!
//! Shared helpers for normalizing Figma REST responses.

use serde_json::{json, Value};

/// Return the document/node payload from a file or file-nodes response.
pub fn response_to_node(data: &Value) -> Value {
    if let Some(nodes) = data.get("nodes").and_then(|n| n.as_object()) {
        let docs: Vec<Value> = nodes
            .values()
            .filter_map(|entry| entry.get("document").cloned())
            .collect();
        if docs.len() == 1 {
            return docs.into_iter().next().unwrap();
        }
        return json!({
            "id": "selection",
            "name": "Selection",
            "type": "SELECTION",
            "children": docs,
        });
    }
    data.get("document").cloned().unwrap_or_else(|| data.clone())
}

/// Lightweight summary of a node.
pub fn node_summary(node: &Value) -> Value {
    let children_count = node
        .get("children")
        .and_then(|c| c.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let mut map = serde_json::Map::new();
    map.insert("id".into(), node.get("id").cloned().unwrap_or(Value::Null));
    map.insert("name".into(), node.get("name").cloned().unwrap_or(Value::Null));
    map.insert("type".into(), node.get("type").cloned().unwrap_or(Value::Null));
    map.insert("children_count".into(), json!(children_count));

    if let Some(bbox) = node.get("absoluteBoundingBox") {
        let w = bbox.get("width");
        let h = bbox.get("height");
        if let (Some(w), Some(h)) = (w, h) {
            if !w.is_null() && !h.is_null() {
                map.insert("dimensions".into(), json!({"width": w, "height": h}));
            }
        }
    }
    Value::Object(map)
}

/// Pick node IDs suitable for screenshot rendering.
pub fn renderable_node_ids(
    node: &Value,
    explicit_node_id: Option<&str>,
    limit: Option<usize>,
) -> Vec<String> {
    let limit = limit.unwrap_or(4);

    if let Some(id) = explicit_node_id {
        return vec![id.to_string()];
    }

    let root_id = node.get("id").and_then(|v| v.as_str());
    let root_type = node.get("type").and_then(|v| v.as_str());

    if let Some(id) = root_id {
        if root_type != Some("DOCUMENT") {
            return vec![id.to_string()];
        }
    }

    node.get("children")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|child| {
                    matches!(
                        child.get("type").and_then(|v| v.as_str()),
                        Some("CANVAS" | "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE")
                    )
                })
                .filter_map(|child| child.get("id").and_then(|v| v.as_str()).map(String::from))
                .take(limit)
                .collect()
        })
        .unwrap_or_default()
}
