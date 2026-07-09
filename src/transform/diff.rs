//! Figma file diff — compare two files or two versions.
//!
//! Returns a structural diff of nodes. Child ordering follows the new tree's
//! child order, which is deterministic.

use serde_json::{json, Map, Value};

fn node_signature(node: &Value) -> Value {
    let mut sig = Map::new();
    for key in ["id", "name", "type", "characters", "visible"] {
        if let Some(v) = node.get(key) {
            sig.insert(key.to_string(), v.clone());
        }
    }
    Value::Object(sig)
}

/// Build an id -> node map preserving the source child order.
fn children_by_id(node: &Value) -> Vec<(String, Value)> {
    node.get("children")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    c.get("id")
                        .and_then(|v| v.as_str())
                        .map(|id| (id.to_string(), c.clone()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn lookup<'a>(pairs: &'a [(String, Value)], id: &str) -> Option<&'a Value> {
    pairs.iter().find(|(k, _)| k == id).map(|(_, v)| v)
}

/// Compare two node trees at one level.
pub fn diff_nodes(old_node: &Value, new_node: &Value) -> Value {
    let old_children = children_by_id(old_node);
    let new_children = children_by_id(new_node);
    let old_ids: Vec<&String> = old_children.iter().map(|(k, _)| k).collect();
    let new_ids: Vec<&String> = new_children.iter().map(|(k, _)| k).collect();

    let added: Vec<Value> = new_children
        .iter()
        .filter(|(k, _)| !old_ids.contains(&k))
        .map(|(_, v)| v.clone())
        .collect();
    let removed: Vec<Value> = old_children
        .iter()
        .filter(|(k, _)| !new_ids.contains(&k))
        .map(|(_, v)| v.clone())
        .collect();

    let common: Vec<&String> = new_children
        .iter()
        .map(|(k, _)| k)
        .filter(|k| old_ids.contains(k))
        .collect();

    let mut modified = Vec::new();
    let mut unchanged = Vec::new();
    for id in &common {
        let old = lookup(&old_children, id).unwrap();
        let new = lookup(&new_children, id).unwrap();
        let old_sig = node_signature(old);
        let new_sig = node_signature(new);
        if old_sig != new_sig {
            modified.push(json!({
                "id": id,
                "name": new.get("name").cloned().unwrap_or(Value::Null),
                "type": new.get("type").cloned().unwrap_or(Value::Null),
                "before": old_sig,
                "after": new_sig,
            }));
        } else {
            unchanged.push(new.get("name").cloned().unwrap_or(Value::Null));
        }
    }

    json!({
        "added": added,
        "removed": removed,
        "modified": modified,
        "unchanged": unchanged,
    })
}

/// Deep recursive diff of two document trees.
pub fn diff_trees(old_tree: &Value, new_tree: &Value) -> Value {
    let mut base = diff_nodes(old_tree, new_tree);
    let old_children = children_by_id(old_tree);
    let new_children = children_by_id(new_tree);
    let old_ids: Vec<&String> = old_children.iter().map(|(k, _)| k).collect();

    let mut child_diffs = Vec::new();
    for (id, new_child) in &new_children {
        if old_ids.contains(&id) {
            let old_child = lookup(&old_children, id).unwrap();
            let mut child_diff = diff_trees(old_child, new_child);
            if let Some(obj) = child_diff.as_object_mut() {
                obj.insert("id".into(), Value::String(id.clone()));
                obj.insert("name".into(), new_child.get("name").cloned().unwrap_or(Value::Null));
                obj.insert("type".into(), new_child.get("type").cloned().unwrap_or(Value::Null));
            }
            child_diffs.push(child_diff);
        }
    }

    if let Some(obj) = base.as_object_mut() {
        obj.insert("children".into(), Value::Array(child_diffs));
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    fn old_tree() -> Value {
        json!({
            "id": "0", "name": "Page", "type": "PAGE",
            "children": [
                {"id": "1", "name": "Header", "type": "FRAME",
                 "children": [{"id": "2", "name": "Title", "type": "TEXT", "characters": "Old"}]},
                {"id": "3", "name": "Removed", "type": "RECTANGLE"}
            ]
        })
    }

    fn new_tree() -> Value {
        json!({
            "id": "0", "name": "Page", "type": "PAGE",
            "children": [
                {"id": "1", "name": "Header", "type": "FRAME",
                 "children": [{"id": "2", "name": "Title", "type": "TEXT", "characters": "New"}]},
                {"id": "4", "name": "Added", "type": "RECTANGLE"}
            ]
        })
    }

    #[test]
    fn reports_added_removed_unchanged() {
        let result = diff_nodes(&old_tree(), &new_tree());
        let added: Vec<&str> = result["added"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
        let removed: Vec<&str> = result["removed"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
        let unchanged: Vec<&str> = result["unchanged"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
        assert_eq!(added, vec!["4"]);
        assert_eq!(removed, vec!["3"]);
        assert_eq!(unchanged, vec!["Header"]);
    }

    #[test]
    fn recurses_into_common_children() {
        let result = diff_trees(&old_tree(), &new_tree());
        let header_diff = &result["children"][0];
        let title_change = &header_diff["modified"][0];
        assert_eq!(header_diff["id"], "1");
        assert_eq!(title_change["id"], "2");
        assert_eq!(title_change["before"]["characters"], "Old");
        assert_eq!(title_change["after"]["characters"], "New");
    }
}
