//! Export Figma file structure as Markdown.
//!
//! Export Figma file structure as Markdown.

use serde_json::Value;

fn indent(level: usize) -> String {
    "  ".repeat(level)
}

/// Render a scalar as its string form (integers plain).
fn scalar_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// Render a small keyword map as `{:key value, ...}` for the layout
/// selection: `{:layoutMode "VERTICAL", :itemSpacing 8}`.
fn pr_str_layout(layout: &Value) -> String {
    let keys = [
        "layoutMode",
        "itemSpacing",
        "paddingLeft",
        "paddingRight",
        "paddingTop",
        "paddingBottom",
    ];
    let mut parts = Vec::new();
    if let Some(obj) = layout.as_object() {
        for k in keys {
            if let Some(v) = obj.get(k) {
                let rendered = match v {
                    Value::String(s) => format!("\"{s}\""),
                    other => scalar_str(other),
                };
                parts.push(format!(":{k} {rendered}"));
            }
        }
    }
    format!("{{{}}}", parts.join(", "))
}

fn array_len(node: &Value, key: &str) -> usize {
    node.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

fn node_to_md(node: &Value, level: usize) -> Vec<String> {
    let heading_level = std::cmp::min(level + 2, 6);
    let heading_prefix = "#".repeat(heading_level);
    let type_label = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");

    let mut lines = vec![format!("{heading_prefix} {name} (`{type_label}`)")];

    if let Some(dims) = node.get("dimensions").filter(|v| !v.is_null()) {
        let w = scalar_str(dims.get("width").unwrap_or(&Value::Null));
        let h = scalar_str(dims.get("height").unwrap_or(&Value::Null));
        lines.push(format!("{}- **Dimensions**: {w}×{h}", indent(level)));
    }
    if let Some(layout) = node.get("layout").filter(|v| !v.is_null()) {
        lines.push(format!(
            "{}- **Layout**: {}",
            indent(level),
            pr_str_layout(layout)
        ));
    }
    if let Some(text) = node.get("characters").and_then(|v| v.as_str()) {
        lines.push(format!("{}- **Text**: \"{text}\"", indent(level)));
    }
    if array_len(node, "fills") > 0 {
        lines.push(format!(
            "{}- **Fills**: {} fill(s)",
            indent(level),
            array_len(node, "fills")
        ));
    }
    if array_len(node, "strokes") > 0 {
        lines.push(format!(
            "{}- **Strokes**: {} stroke(s)",
            indent(level),
            array_len(node, "strokes")
        ));
    }
    if node.get("truncated") == Some(&Value::Bool(true)) {
        lines.push(format!("{}- ⚠️ **Truncated**", indent(level)));
    }
    lines
}

fn traverse_to_md(node: &Value, level: usize) -> Vec<String> {
    let mut lines = node_to_md(node, level);
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        if !children.is_empty() {
            for child in children {
                lines.extend(traverse_to_md(child, level + 1));
            }
        }
    }
    lines
}

/// Convert a Figma tree to a Markdown document.
pub fn tree_to_markdown(tree: &Value, title: Option<&str>) -> String {
    let resolved_title = title
        .filter(|t| !t.is_empty())
        .map(String::from)
        .or_else(|| tree.get("name").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_else(|| "Figma Design Document".to_string());
    let header = format!("# {resolved_title}\n\n");
    let body = traverse_to_md(tree, 0).join("\n");
    format!("{header}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tree() -> Value {
        json!({
            "id": "0:1", "name": "My Design", "type": "PAGE",
            "dimensions": {"width": 375, "height": 812},
            "children": [{
                "id": "1:1", "name": "Header", "type": "FRAME",
                "dimensions": {"width": 375, "height": 60},
                "layout": {"layoutMode": "HORIZONTAL", "itemSpacing": 8},
                "children": [
                    {"id": "1:2", "name": "Logo", "type": "VECTOR"},
                    {"id": "1:3", "name": "Title", "type": "TEXT", "characters": "App Name"}
                ]
            }]
        })
    }

    #[test]
    fn generates_markdown_headings() {
        let result = tree_to_markdown(&sample_tree(), Some("Test"));
        assert!(result.contains("# Test"));
        assert!(result.contains("## Header"));
        assert!(result.contains("### Logo"));
    }

    #[test]
    fn includes_dimensions() {
        let result = tree_to_markdown(&sample_tree(), None);
        assert!(result.contains("375×812"));
    }

    #[test]
    fn includes_text_content() {
        let result = tree_to_markdown(&sample_tree(), None);
        assert!(result.contains("App Name"));
    }
}
