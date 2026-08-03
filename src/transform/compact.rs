//! JSON simplification engine with token-budget-aware truncation.
//!
//! All operations are immutable — the input value is
//! never modified. Node maps use `serde_json`'s `preserve_order` feature so
//! field insertion order follows the extractor pipeline.

use serde_json::{Map, Value, json};

/// Rough token estimate for a JSON value.
///
/// The estimate divides the compact JSON string length by 3.5: the metric is
/// monotonic in size and always positive, which is all the truncation logic and
/// tests depend on.
pub fn estimate_tokens(data: &Value) -> i64 {
    let len = serde_json::to_string(data).map(|s| s.len()).unwrap_or(0);
    std::cmp::max(1, (len as f64 / 3.5) as i64)
}

fn str_field<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get(key).and_then(|v| v.as_str())
}

fn has_children(node: &Value) -> bool {
    node.get("children")
        .and_then(|c| c.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

fn node_type(node: &Value) -> &str {
    str_field(node, "type").unwrap_or("")
}

/// Self-score: how valuable is this node for AI consumption?
fn node_info_score(node: &Value) -> i64 {
    let type_score = match node_type(node) {
        "FRAME" | "COMPONENT" | "COMPONENT_SET" => 20,
        "INSTANCE" => 15,
        "TEXT" => 12,
        "RECTANGLE" | "ELLIPSE" | "STAR" | "LINE" | "REGULAR_POLYGON" => 5,
        "VECTOR" | "BOOLEAN_OPERATION" => 2,
        _ => 1,
    };
    let leaf_bonus = if has_children(node) { 0 } else { 3 };
    let text_bonus = if node
        .get("characters")
        .and_then(|c| c.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        5
    } else {
        0
    };
    type_score + leaf_bonus + text_bonus
}

/// Post-order scoring. Returns the scored node and its total score.
/// Each node gains `_score` = self-score + sum(children scores).
pub fn score_tree(node: &Value) -> (Value, i64) {
    if has_children(node) {
        let children = node["children"].as_array().unwrap();
        let mut scored_children = Vec::with_capacity(children.len());
        let mut child_total = 0i64;
        for child in children {
            let (sc, total) = score_tree(child);
            child_total += total;
            scored_children.push(sc);
        }
        let self_score = node_info_score(node);
        let total = self_score + child_total;
        let mut obj = node.as_object().cloned().unwrap_or_default();
        obj.insert("children".into(), Value::Array(scored_children));
        obj.insert("_score".into(), json!(total));
        (Value::Object(obj), total)
    } else {
        let score = node_info_score(node);
        let mut obj = node.as_object().cloned().unwrap_or_default();
        obj.insert("_score".into(), json!(score));
        (Value::Object(obj), score)
    }
}

// --- Extractors ---

fn is_visible_fill(fill: &Value) -> bool {
    match fill.get("visible") {
        Some(Value::String(s)) => s == "VISIBLE",
        Some(Value::Null) | None => true,
        // (or (:visible fill) "VISIBLE") — any non-nil non-"VISIBLE" value fails.
        Some(_) => false,
    }
}

fn select_keys(node: &Value, keys: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    if let Some(obj) = node.as_object() {
        for k in keys {
            if let Some(v) = obj.get(*k) {
                out.insert((*k).to_string(), v.clone());
            }
        }
    }
    out
}

fn clean_fills(fills: &Value) -> Option<Value> {
    let arr = fills.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for fill in arr {
        if !is_visible_fill(fill) {
            continue;
        }
        let mut base = select_keys(fill, &["type", "opacity"]);
        match fill.get("type").and_then(|v| v.as_str()) {
            Some("SOLID") => {
                let color = select_keys(
                    fill.get("color").unwrap_or(&Value::Null),
                    &["r", "g", "b", "a"],
                );
                base.insert("color".into(), Value::Object(color));
            }
            Some("GRADIENT_LINEAR")
            | Some("GRADIENT_RADIAL")
            | Some("GRADIENT_ANGULAR")
            | Some("GRADIENT_DIAMOND") => {
                base.insert(
                    "gradientHandlePositions".into(),
                    fill.get("gradientHandlePositions")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                base.insert(
                    "gradientStops".into(),
                    fill.get("gradientStops").cloned().unwrap_or(Value::Null),
                );
            }
            Some("IMAGE") => {
                base.insert(
                    "imageRef".into(),
                    fill.get("imageRef").cloned().unwrap_or(Value::Null),
                );
            }
            _ => {}
        }
        out.push(Value::Object(base));
    }
    if out.is_empty() {
        None
    } else {
        Some(Value::Array(out))
    }
}

fn clean_strokes(strokes: &Value) -> Option<Value> {
    let arr = strokes.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for stroke in arr {
        if !is_visible_fill(stroke) {
            continue;
        }
        let mut base = select_keys(stroke, &["type", "strokeWeight", "strokeAlign"]);
        if stroke.get("type").and_then(|v| v.as_str()) == Some("SOLID") {
            let color = select_keys(
                stroke.get("color").unwrap_or(&Value::Null),
                &["r", "g", "b", "a"],
            );
            base.insert("color".into(), Value::Object(color));
        }
        out.push(Value::Object(base));
    }
    if out.is_empty() {
        None
    } else {
        Some(Value::Array(out))
    }
}

fn layout_extractor(node: &Value) -> Option<Map<String, Value>> {
    if !matches!(node_type(node), "FRAME" | "COMPONENT" | "INSTANCE") {
        return None;
    }
    let layout = select_keys(
        node,
        &[
            "layoutMode",
            "layoutAlign",
            "layoutGrow",
            "layoutPositioning",
            "layoutSizingHorizontal",
            "layoutSizingVertical",
            "itemSpacing",
            "counterAxisSpacing",
            "counterAxisAlignItems",
            "primaryAxisAlignItems",
            "paddingLeft",
            "paddingRight",
            "paddingTop",
            "paddingBottom",
        ],
    );
    if layout.is_empty() {
        None
    } else {
        let mut m = Map::new();
        m.insert("layout".into(), Value::Object(layout));
        Some(m)
    }
}

fn text_extractor(node: &Value) -> Option<Map<String, Value>> {
    if node_type(node) != "TEXT" {
        return None;
    }
    let base = select_keys(node, &["characters"]);
    if base.is_empty() {
        return None;
    }
    let mut m = base;
    let text_style = select_keys(
        node.get("style").unwrap_or(&Value::Null),
        &[
            "fontFamily",
            "fontSize",
            "fontWeight",
            "textAlignHorizontal",
            "textAlignVertical",
            "letterSpacing",
            "lineHeightPx",
            "lineHeightPercentFontSize",
        ],
    );
    m.insert("textStyle".into(), Value::Object(text_style));
    Some(m)
}

fn visuals_extractor(node: &Value) -> Option<Map<String, Value>> {
    let mut visuals = Map::new();
    if let Some(fills) = node.get("fills").and_then(clean_fills) {
        visuals.insert("fills".into(), fills);
    }
    if let Some(strokes) = node.get("strokes").and_then(clean_strokes) {
        visuals.insert("strokes".into(), strokes);
    }
    if let Some(effects) = node.get("effects").and_then(|e| e.as_array()) {
        let cleaned: Vec<Value> = effects
            .iter()
            .filter(|ef| is_visible_fill(ef))
            .map(|ef| {
                Value::Object(select_keys(
                    ef,
                    &["type", "color", "offset", "radius", "spread"],
                ))
            })
            .collect();
        if !cleaned.is_empty() {
            visuals.insert("effects".into(), Value::Array(cleaned));
        }
    }
    if let Some(op) = node.get("opacity") {
        if !op.is_null() {
            visuals.insert("opacity".into(), op.clone());
        }
    }
    if let Some(cr) = node.get("cornerRadius") {
        if !cr.is_null() {
            visuals.insert("cornerRadius".into(), cr.clone());
        }
    }
    if let Some(rc) = node.get("rectangleCornerRadii") {
        if !rc.is_null() {
            visuals.insert("rectangleCornerRadii".into(), rc.clone());
        }
    }
    if visuals.is_empty() {
        None
    } else {
        Some(visuals)
    }
}

fn dimension_extractor(node: &Value) -> Option<Map<String, Value>> {
    let bbox = node.get("absoluteBoundingBox")?;
    if bbox.is_null() {
        return None;
    }
    let width = bbox.get("width").cloned().unwrap_or(Value::Null);
    let height = bbox.get("height").cloned().unwrap_or(Value::Null);
    let mut dims = Map::new();
    dims.insert("width".into(), width);
    dims.insert("height".into(), height);
    let mut m = Map::new();
    m.insert("dimensions".into(), Value::Object(dims));
    Some(m)
}

fn component_extractor(node: &Value) -> Option<Map<String, Value>> {
    if node_type(node) != "INSTANCE" {
        return None;
    }
    Some(select_keys(node, &["componentId", "componentProperties"]))
}

/// An extractor pulls a partial map out of a node during simplification.
pub type Extractor = fn(&Value) -> Option<Map<String, Value>>;

/// The default extractor pipeline (layout, text, visuals, dimension, component).
pub const DEFAULT_EXTRACTORS: &[Extractor] = &[
    layout_extractor,
    text_extractor,
    visuals_extractor,
    dimension_extractor,
    component_extractor,
];

/// The lightweight `file tree` extractor set (dimension, then layout).
pub const TREE_EXTRACTORS: &[Extractor] = &[dimension_extractor, layout_extractor];

/// Apply an extractor pipeline to a single node.
pub fn simplify_node_with(node: &Value, extractors: &[Extractor]) -> Value {
    let mut out = Map::new();
    out.insert("id".into(), node.get("id").cloned().unwrap_or(Value::Null));
    out.insert(
        "name".into(),
        node.get("name").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "type".into(),
        node.get("type").cloned().unwrap_or(Value::Null),
    );

    for extractor in extractors {
        if let Some(data) = extractor(node) {
            for (k, v) in data {
                out.insert(k, v);
            }
        }
    }
    Value::Object(out)
}

/// Apply the default extractor pipeline to a single node.
pub fn simplify_node(node: &Value) -> Value {
    simplify_node_with(node, DEFAULT_EXTRACTORS)
}

fn should_traverse(node: &Value, max_depth: Option<i64>, current_depth: i64) -> bool {
    let depth_ok = max_depth.map(|m| current_depth < m).unwrap_or(true);
    let visible = node.get("visible") != Some(&Value::Bool(false));
    depth_ok && has_children(node) && visible
}

/// Recursively simplify a node tree with the default extractors.
pub fn simplify_tree(node: &Value, max_depth: Option<i64>) -> Value {
    simplify_tree_with(node, max_depth, DEFAULT_EXTRACTORS)
}

/// Recursively simplify a node tree with a specific extractor set.
pub fn simplify_tree_with(node: &Value, max_depth: Option<i64>, extractors: &[Extractor]) -> Value {
    simplify_tree_inner(node, max_depth, 0, extractors)
}

fn simplify_tree_inner(
    node: &Value,
    max_depth: Option<i64>,
    current_depth: i64,
    extractors: &[Extractor],
) -> Value {
    let mut base = simplify_node_with(node, extractors);
    if should_traverse(node, max_depth, current_depth) {
        let children: Vec<Value> = node["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| simplify_tree_inner(c, max_depth, current_depth + 1, extractors))
            .collect();
        if let Some(obj) = base.as_object_mut() {
            obj.insert("children".into(), Value::Array(children));
        }
    }
    base
}

// --- Token-aware truncation ---

/// Collect all nodes with their child-index path (root path is empty).
fn all_subtrees<'a>(node: &'a Value, path: Vec<usize>, out: &mut Vec<(Vec<usize>, &'a Value)>) {
    out.push((path.clone(), node));
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for (i, child) in children.iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(i);
            all_subtrees(child, child_path, out);
        }
    }
}

fn get_at<'a>(tree: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut cur = tree;
    for &i in path {
        cur = cur.get("children")?.get(i)?;
    }
    Some(cur)
}

fn truncation_marker(node: &Value) -> Value {
    json!({
        "id": node.get("id").cloned().unwrap_or(Value::Null),
        "name": node.get("name").cloned().unwrap_or(Value::Null),
        "type": node.get("type").cloned().unwrap_or(Value::Null),
        "truncated": true,
    })
}

/// Replace the subtree at `path` with a truncation marker.
fn truncation_saving(node: &Value) -> i64 {
    estimate_tokens(node) - estimate_tokens(&truncation_marker(node))
}

struct Candidate {
    path: Vec<usize>,
    score: i64,
    saving: i64,
}

fn truncation_candidates(scored: &Value) -> Vec<Candidate> {
    let mut all = Vec::new();
    all_subtrees(scored, Vec::new(), &mut all);
    // Skip root.
    let mut candidates: Vec<Candidate> = all
        .into_iter()
        .skip(1)
        .filter_map(|(path, node)| {
            let saving = truncation_saving(node);
            if saving > 0 {
                Some(Candidate {
                    score: node_info_score(node),
                    saving,
                    path,
                })
            } else {
                None
            }
        })
        .collect();
    // sort-by [score, -saving, -path-len] ascending.
    candidates.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then((-a.saving).cmp(&(-b.saving)))
            .then((-(a.path.len() as i64)).cmp(&(-(b.path.len() as i64))))
    });
    candidates
}

/// Smart truncation: remove lowest-score subtrees until within budget.
pub fn truncate_by_budget(node: &Value, max_tokens: i64) -> Value {
    let (scored, _) = score_tree(node);
    let initial = estimate_tokens(&scored);
    if initial <= max_tokens {
        return scored;
    }

    let candidates = truncation_candidates(&scored);
    let mut current = scored;
    let mut removed: std::collections::HashSet<Vec<usize>> = std::collections::HashSet::new();
    let mut idx = 0usize;
    let mut iterations = 0i64;

    loop {
        let tokens = estimate_tokens(&current);
        if tokens <= max_tokens || idx >= candidates.len() || iterations > 1000 {
            return current;
        }
        let cand = &candidates[idx];
        idx += 1;
        if removed.contains(&cand.path) || get_at(&current, &cand.path).is_none() {
            continue;
        }
        set_truncation_marker(&mut current, &cand.path);
        removed.insert(cand.path.clone());
        iterations += 1;
    }
}

/// Replace the node at the given child-index path with its truncation marker.
fn set_truncation_marker(tree: &mut Value, path: &[usize]) {
    // Navigate to the node, compute its marker, then assign.
    let marker = match get_at(tree, path) {
        Some(node) => truncation_marker(node),
        None => return,
    };
    let mut cur = tree;
    for (depth, &i) in path.iter().enumerate() {
        let children = match cur.get_mut("children").and_then(|c| c.as_array_mut()) {
            Some(c) => c,
            None => return,
        };
        if i >= children.len() {
            return;
        }
        if depth == path.len() - 1 {
            children[i] = marker;
            return;
        }
        cur = &mut children[i];
    }
}

/// Full pipeline: simplify + optional token truncation.
pub fn compact(node: &Value, max_depth: Option<i64>, max_tokens: Option<i64>) -> Value {
    let simplified = simplify_tree(node, max_depth);
    match max_tokens {
        Some(mt) => truncate_by_budget(&simplified, mt),
        None => simplified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_node() -> Value {
        json!({
            "id": "1:1",
            "name": "Frame",
            "type": "FRAME",
            "absoluteBoundingBox": {"width": 100, "height": 200},
            "layoutMode": "VERTICAL",
            "fills": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0}, "visible": "VISIBLE"}],
            "children": [
                {"id": "1:2", "name": "Text", "type": "TEXT", "characters": "Hello", "style": {"fontSize": 16}},
                {"id": "1:3", "name": "Rect", "type": "RECTANGLE", "absoluteBoundingBox": {"width": 50, "height": 50}}
            ]
        })
    }

    #[test]
    fn simplify_node_extracts_key_fields() {
        let r = simplify_node(&sample_node());
        assert_eq!(r["id"], "1:1");
        assert_eq!(r["name"], "Frame");
        assert_eq!(r["type"], "FRAME");
        assert!(r.get("layout").is_some());
        assert!(r.get("dimensions").is_some());
    }

    #[test]
    fn text_extractor_captures_characters_and_style() {
        let text = json!({"id": "1:2", "name": "Text", "type": "TEXT", "characters": "Hello", "style": {"fontSize": 16}});
        let r = simplify_node(&text);
        assert_eq!(r["characters"], "Hello");
        assert_eq!(r["textStyle"]["fontSize"], 16);
    }

    #[test]
    fn simplify_tree_preserves_hierarchy() {
        let r = simplify_tree(&sample_node(), Some(2));
        assert_eq!(r["type"], "FRAME");
        assert_eq!(r["children"].as_array().unwrap().len(), 2);
        assert_eq!(r["children"][0]["type"], "TEXT");
    }

    #[test]
    fn depth_limit_stops_traversal() {
        let r = simplify_tree(&sample_node(), Some(0));
        assert!(r.get("children").is_none());
    }

    #[test]
    fn estimate_tokens_positive_and_monotonic() {
        assert!(estimate_tokens(&json!({"a": 1, "b": 2})) > 0);
        assert!(
            estimate_tokens(&json!({"a": 1, "b": 2, "c": 3})) > estimate_tokens(&json!({"a": 1}))
        );
    }

    #[test]
    fn score_tree_adds_score() {
        let (scored, _) = score_tree(&sample_node());
        assert!(scored["_score"].as_i64().unwrap() > 0);
        assert!(scored["children"].is_array());
    }

    #[test]
    fn all_subtrees_paths() {
        let tree = json!({
            "children": [
                {"id": "1", "name": "A", "type": "FRAME", "children": [{"id": "2", "name": "B", "type": "TEXT"}]},
                {"id": "3", "name": "C", "type": "RECTANGLE"}
            ]
        });
        let (scored, _) = score_tree(&tree);
        let mut subs = Vec::new();
        all_subtrees(&scored, Vec::new(), &mut subs);
        assert_eq!(subs.len(), 4);
        assert_eq!(subs[0].0, Vec::<usize>::new());
        assert_eq!(subs[1].0, vec![0]);
        assert_eq!(subs[2].0, vec![0, 0]);
        assert_eq!(subs[3].0, vec![1]);
    }

    #[test]
    fn truncate_at_path_replaces_subtree() {
        let tree = json!({
            "children": [
                {"id": "1", "name": "A", "type": "FRAME", "children": [{"id": "2", "name": "B", "type": "TEXT"}]},
                {"id": "3", "name": "C", "type": "RECTANGLE"}
            ]
        });
        let (mut scored, _) = score_tree(&tree);
        set_truncation_marker(&mut scored, &[0]);
        assert_eq!(scored["children"].as_array().unwrap().len(), 2);
        assert_eq!(scored["children"][0]["truncated"], true);
        assert_eq!(scored["children"][0]["name"], "A");
        assert_eq!(scored["children"][1]["name"], "C");
    }

    #[test]
    fn tree_within_budget_not_truncated() {
        let tree = simplify_tree(&sample_node(), Some(2));
        let result = truncate_by_budget(&tree, 100_000);
        assert!(result.get("truncated").is_none());
    }

    #[test]
    fn tree_exceeding_budget_gets_truncated() {
        let mut children = Vec::new();
        for i in 0..20 {
            children.push(json!({
                "id": format!("1:{i}"), "name": format!("Frame {i}"), "type": "FRAME",
                "absoluteBoundingBox": {"width": 100, "height": 100},
                "layout": {"layoutMode": "VERTICAL"},
                "fills": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0}}],
                "children": [{"id": format!("2:{i}"), "name": format!("Text {i}"), "type": "TEXT", "characters": format!("Hello {i}")}]
            }));
        }
        let big = json!({"id": "0:1", "name": "Page", "type": "PAGE", "children": children});
        let tree = simplify_tree(&big, Some(2));
        let original = estimate_tokens(&tree);
        let result = truncate_by_budget(&tree, 100);
        let truncated_any = result["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c.get("truncated") == Some(&Value::Bool(true)));
        assert!(truncated_any);
        assert!(estimate_tokens(&result) < original);
    }

    #[test]
    fn truncate_score_priority() {
        let mut decor_children = Vec::new();
        for i in 0..10 {
            decor_children.push(
                json!({"id": format!("d{i}"), "name": format!("Tiny {i}"), "type": "VECTOR"}),
            );
        }
        let tree = json!({
            "id": "0", "name": "Root", "type": "FRAME",
            "children": [
                {"id": "1", "name": "Decor", "type": "VECTOR",
                 "fills": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0}}],
                 "children": decor_children},
                {"id": "2", "name": "Copy", "type": "TEXT", "characters": "Important call to action"}
            ]
        });
        let budget = estimate_tokens(&tree) - 20;
        let result = truncate_by_budget(&tree, budget);
        let decor_truncated = result["children"][0].get("truncated") == Some(&Value::Bool(true))
            || result["children"][0]["children"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .any(|c| c.get("truncated") == Some(&Value::Bool(true)))
                })
                .unwrap_or(false);
        assert!(decor_truncated);
        assert_ne!(
            result["children"][1].get("truncated"),
            Some(&Value::Bool(true))
        );
    }
}
