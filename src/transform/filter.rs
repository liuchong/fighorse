//! Tree filtering utilities for AI-oriented Figma context selection.
//!
//! Tree filtering utilities for AI-oriented Figma context selection.

use serde_json::Value;
use std::collections::HashSet;

/// Minimum-size constraint parsed from a `WxH` string.
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// Filtering options.
#[derive(Debug, Clone, Default)]
pub struct FilterOpts {
    pub types: HashSet<String>,
    pub name_regex: Option<String>,
    pub visible_only: bool,
    pub min_size: Option<Size>,
    pub has_fill: bool,
    pub no_children: bool,
}

/// Parse a `WxH` size string (e.g. `10x20`). Returns `None` when malformed.
pub fn parse_size(s: &str) -> Option<Size> {
    if s.is_empty() {
        return None;
    }
    let mut parts = s.split('x');
    let w = parts.next()?.parse::<f64>().ok()?;
    let h = parts.next()?.parse::<f64>().ok()?;
    Some(Size {
        width: w,
        height: h,
    })
}

/// Parse a comma-separated type list into a set.
pub fn parse_types(s: &str) -> HashSet<String> {
    if s.trim().is_empty() {
        return HashSet::new();
    }
    s.split(',').map(|t| t.trim().to_string()).collect()
}

fn dimensions(node: &Value) -> (Option<f64>, Option<f64>) {
    if let Some(dims) = node.get("dimensions") {
        return (
            dims.get("width").and_then(|v| v.as_f64()),
            dims.get("height").and_then(|v| v.as_f64()),
        );
    }
    if let Some(bbox) = node.get("absoluteBoundingBox") {
        return (
            bbox.get("width").and_then(|v| v.as_f64()),
            bbox.get("height").and_then(|v| v.as_f64()),
        );
    }
    (None, None)
}

fn visible(node: &Value) -> bool {
    node.get("visible") != Some(&Value::Bool(false))
}

fn type_match(node: &Value, types: &HashSet<String>) -> bool {
    if types.is_empty() {
        return true;
    }
    node.get("type")
        .and_then(|v| v.as_str())
        .map(|t| types.contains(t))
        .unwrap_or(false)
}

fn name_match(node: &Value, name_regex: &Option<String>) -> bool {
    match name_regex {
        None => true,
        Some(re) if re.is_empty() => true,
        Some(re) => {
            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
            regex::RegexBuilder::new(re)
                .case_insensitive(true)
                .build()
                .map(|r| r.is_match(name))
                .unwrap_or(false)
        }
    }
}

fn size_match(node: &Value, min_size: &Option<Size>) -> bool {
    match min_size {
        None => true,
        Some(min) => {
            let (w, h) = dimensions(node);
            match (w, h) {
                (Some(w), Some(h)) => w >= min.width && h >= min.height,
                _ => false,
            }
        }
    }
}

fn has_fills(node: &Value) -> bool {
    node.get("fills")
        .and_then(|f| f.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

fn has_children(node: &Value) -> bool {
    node.get("children")
        .and_then(|c| c.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

fn node_match(node: &Value, opts: &FilterOpts) -> bool {
    type_match(node, &opts.types)
        && name_match(node, &opts.name_regex)
        && (!opts.visible_only || visible(node))
        && size_match(node, &opts.min_size)
        && (!opts.has_fill || has_fills(node))
        && (!opts.no_children || !has_children(node))
}

/// Filter a tree, preserving ancestors of matching descendants.
pub fn filter_tree(node: &Value, opts: &FilterOpts) -> Option<Value> {
    let kept_children: Vec<Value> = node
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(|c| filter_tree(c, opts)).collect())
        .unwrap_or_default();

    let self_match = node_match(node, opts);
    if self_match || !kept_children.is_empty() {
        let mut obj = node.as_object().cloned().unwrap_or_default();
        obj.remove("children");
        if !kept_children.is_empty() {
            obj.insert("children".into(), Value::Array(kept_children));
        }
        Some(Value::Object(obj))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tree() -> Value {
        json!({
            "id": "0", "name": "Page", "type": "PAGE",
            "children": [
                {"id": "1", "name": "Login Card", "type": "FRAME",
                 "dimensions": {"width": 320, "height": 200},
                 "children": [
                     {"id": "2", "name": "Title", "type": "TEXT", "visible": true},
                     {"id": "3", "name": "Hidden", "type": "TEXT", "visible": false}
                 ]},
                {"id": "4", "name": "Dot", "type": "RECTANGLE", "dimensions": {"width": 2, "height": 2}}
            ]
        })
    }

    #[test]
    fn parse_size_ok() {
        let s = parse_size("10x20").unwrap();
        assert_eq!(s.width, 10.0);
        assert_eq!(s.height, 20.0);
    }

    #[test]
    fn parse_size_invalid() {
        assert!(parse_size("bad").is_none());
    }

    #[test]
    fn keeps_matching_descendants_and_ancestors() {
        let opts = FilterOpts {
            types: parse_types("TEXT"),
            visible_only: true,
            ..Default::default()
        };
        let result = filter_tree(&sample_tree(), &opts).unwrap();
        assert_eq!(result["name"], "Page");
        assert_eq!(result["children"][0]["name"], "Login Card");
        let names: Vec<&str> = result["children"][0]["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["Title"]);
    }

    #[test]
    fn filters_by_min_size() {
        let opts = FilterOpts {
            min_size: Some(Size {
                width: 100.0,
                height: 100.0,
            }),
            ..Default::default()
        };
        let result = filter_tree(&sample_tree(), &opts).unwrap();
        let names: Vec<&str> = result["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["Login Card"]);
    }
}
