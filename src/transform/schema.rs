//! Component schema inference from Figma component nodes.
//!
//! Component schema inference from Figma component nodes.

use serde_json::{json, Value};

/// Depth-first search for a node by id.
pub fn find_node<'a>(node: &'a Value, node_id: &str) -> Option<&'a Value> {
    if node.is_null() {
        return None;
    }
    if node.get("id").and_then(|v| v.as_str()) == Some(node_id) {
        return Some(node);
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            if let Some(found) = find_node(child, node_id) {
                return Some(found);
            }
        }
    }
    None
}

fn sanitize_name(s: Option<&str>) -> String {
    let raw = s.unwrap_or("Component");
    // Replace non [A-Za-z0-9_ ] with space, collapse whitespace, trim.
    let non_allowed = regex::Regex::new(r"[^A-Za-z0-9_ ]").unwrap();
    let step1 = non_allowed.replace_all(raw, " ");
    let ws = regex::Regex::new(r"\s+").unwrap();
    let step2 = ws.replace_all(&step1, " ");
    let base = step2.trim();
    if base.is_empty() {
        "Component".to_string()
    } else {
        base.replace(' ', "")
    }
}

fn infer_property_type(prop: &Value) -> String {
    match prop.get("type").and_then(|v| v.as_str()) {
        Some("BOOLEAN") => "boolean".to_string(),
        Some("TEXT") => "string".to_string(),
        Some("INSTANCE_SWAP") => "string".to_string(),
        Some("VARIANT") => match prop.get("variantOptions").and_then(|v| v.as_array()) {
            Some(options) if !options.is_empty() => options
                .iter()
                .map(|o| format!("\"{}\"", o.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(" | "),
            _ => "string".to_string(),
        },
        _ => "string".to_string(),
    }
}

/// Inferred component schema.
pub struct Schema {
    pub value: Value,
}

/// Infer a component schema (props + interface name) for a node id.
pub fn infer_component_schema(tree: &Value, component_id: &str) -> Option<Value> {
    let node = find_node(tree, component_id)?;
    let name = node.get("name").and_then(|v| v.as_str());
    let interface = format!("{}Props", sanitize_name(name));

    // componentProperties is an object; preserve insertion order.
    let props: Vec<Value> = node
        .get("componentProperties")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(pname, prop)| {
                    json!({
                        "name": pname,
                        "type": infer_property_type(prop),
                        "default": prop.get("value").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(json!({
        "component": {
            "id": node.get("id").cloned().unwrap_or(Value::Null),
            "name": node.get("name").cloned().unwrap_or(Value::Null),
            "type": node.get("type").cloned().unwrap_or(Value::Null),
        },
        "interface": interface,
        "props": props,
    }))
}

/// Render an inferred schema as a TypeScript interface.
pub fn schema_to_typescript(schema: &Value) -> String {
    let interface = schema
        .get("interface")
        .and_then(|v| v.as_str())
        .unwrap_or("Props");
    let mut lines = vec![format!("export interface {interface} {{")];
    if let Some(props) = schema.get("props").and_then(|v| v.as_array()) {
        for prop in props {
            let name = prop.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let ty = prop
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("string");
            lines.push(format!("  \"{name}\"?: {ty};"));
        }
    }
    lines.push("}".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component_tree() -> Value {
        json!({
            "id": "0", "name": "Page", "type": "PAGE",
            "children": [
                {"id": "1", "name": "Button/Primary", "type": "COMPONENT",
                 "componentProperties": {
                     "Disabled": {"type": "BOOLEAN", "value": false},
                     "Label": {"type": "TEXT", "value": "Submit"},
                     "Size": {"type": "VARIANT", "value": "md", "variantOptions": ["sm", "md", "lg"]}
                 }}
            ]
        })
    }

    #[test]
    fn infers_component_props() {
        let result = infer_component_schema(&component_tree(), "1").unwrap();
        assert_eq!(result["interface"], "ButtonPrimaryProps");
        assert_eq!(result["props"].as_array().unwrap().len(), 3);
        assert_eq!(result["props"][0]["type"], "boolean");
    }

    #[test]
    fn renders_typescript() {
        let schema = infer_component_schema(&component_tree(), "1").unwrap();
        let ts = schema_to_typescript(&schema);
        assert!(ts.contains("export interface ButtonPrimaryProps"));
        assert!(ts.contains("\"Disabled\"?: boolean;"));
    }
}
