//! Design token extraction from Figma files.
//!
//! Design token extraction: extracts colors, typography, spacing, and effects
//! as design tokens and formats them to CSS / SCSS / Tailwind / JSON.

use serde_json::{json, Map, Value};

fn clamp_channel(v: Option<f64>) -> i64 {
    let scaled = (v.unwrap_or(0.0) * 255.0).round() as i64;
    scaled.clamp(0, 255)
}

fn channel_to_hex(v: Option<f64>) -> String {
    format!("{:02x}", clamp_channel(v))
}

fn color_to_hex(color: &Value) -> Option<String> {
    let r = color.get("r").and_then(|v| v.as_f64());
    let g = color.get("g").and_then(|v| v.as_f64());
    let b = color.get("b").and_then(|v| v.as_f64());
    if r.is_some() && g.is_some() && b.is_some() {
        Some(format!(
            "#{}{}{}",
            channel_to_hex(r),
            channel_to_hex(g),
            channel_to_hex(b)
        ))
    } else {
        None
    }
}

fn node_name(node: &Value) -> Value {
    node.get("name").cloned().unwrap_or(Value::Null)
}

fn extract_colors(node: &Value, tokens: &mut Vec<Value>) {
    if let Some(fills) = node.get("fills").and_then(|f| f.as_array()) {
        for fill in fills {
            if fill.get("type").and_then(|v| v.as_str()) == Some("SOLID") {
                let color = fill.get("color").cloned().unwrap_or(Value::Null);
                let r = color.get("r").cloned().unwrap_or(Value::Null);
                let g = color.get("g").cloned().unwrap_or(Value::Null);
                let b = color.get("b").cloned().unwrap_or(Value::Null);
                let a = color.get("a").cloned().unwrap_or(json!(1));
                tokens.push(json!({
                    "name": node_name(node),
                    "type": "color",
                    "value": {"r": r, "g": g, "b": b, "a": a},
                    "hex": color_to_hex(&color),
                }));
            }
        }
    }
}

fn select_keys(obj: &Value, keys: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    if let Some(o) = obj.as_object() {
        for k in keys {
            if let Some(v) = o.get(*k) {
                out.insert((*k).to_string(), v.clone());
            }
        }
    }
    out
}

fn extract_typography(node: &Value, tokens: &mut Vec<Value>) {
    if node.get("type").and_then(|v| v.as_str()) != Some("TEXT") {
        return;
    }
    let style = node
        .get("textStyle")
        .filter(|v| !v.is_null())
        .or_else(|| node.get("style").filter(|v| !v.is_null()));
    if let Some(style) = style {
        let value = select_keys(
            style,
            &[
                "fontFamily",
                "fontSize",
                "fontWeight",
                "textAlignHorizontal",
                "textAlignVertical",
                "letterSpacing",
                "lineHeightPx",
            ],
        );
        tokens.push(json!({
            "name": node_name(node),
            "type": "typography",
            "value": Value::Object(value),
        }));
    }
}

fn extract_spacing(node: &Value, tokens: &mut Vec<Value>) {
    let layout = node.get("layout").cloned().unwrap_or(Value::Null);
    let value = select_keys(
        &layout,
        &[
            "itemSpacing",
            "counterAxisSpacing",
            "paddingLeft",
            "paddingRight",
            "paddingTop",
            "paddingBottom",
        ],
    );
    if !value.is_empty() {
        tokens.push(json!({
            "name": node_name(node),
            "type": "spacing",
            "value": Value::Object(value),
        }));
    }
}

fn extract_effects(node: &Value, tokens: &mut Vec<Value>) {
    if let Some(effects) = node.get("effects").and_then(|e| e.as_array()) {
        for effect in effects {
            if effect.get("type").and_then(|v| v.as_str()) == Some("DROP_SHADOW") {
                let value = select_keys(effect, &["type", "color", "offset", "radius", "spread"]);
                tokens.push(json!({
                    "name": node_name(node),
                    "type": "shadow",
                    "value": Value::Object(value),
                }));
            }
        }
    }
}

/// Extract design tokens from a (possibly simplified) Figma tree.
pub fn extract_tokens(tree: &Value) -> Vec<Value> {
    let mut tokens = Vec::new();
    extract_into(tree, &mut tokens);
    tokens
}

fn extract_into(tree: &Value, tokens: &mut Vec<Value>) {
    extract_colors(tree, tokens);
    extract_typography(tree, tokens);
    extract_spacing(tree, tokens);
    extract_effects(tree, tokens);
    if let Some(children) = tree.get("children").and_then(|c| c.as_array()) {
        for child in children {
            extract_into(child, tokens);
        }
    }
}

/// Group extracted tokens by their `type`.
pub fn tokens_by_category(tokens: &[Value]) -> Value {
    let mut grouped: Map<String, Value> = Map::new();
    for t in tokens {
        let ty = t
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        grouped
            .entry(ty)
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .unwrap()
            .push(t.clone());
    }
    Value::Object(grouped)
}

fn slug_name(name: &Value) -> String {
    let raw = name.as_str().unwrap_or("token").to_lowercase();
    let non_alnum = regex::Regex::new(r"[^a-z0-9]+").unwrap();
    let step = non_alnum.replace_all(&raw, "-");
    let trim = regex::Regex::new(r"(^-|-$)").unwrap();
    trim.replace_all(&step, "").into_owned()
}

fn token_key(prefix: &str, token: &Value, suffix: &str) -> String {
    let ty = token.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let name_part = slug_name(token.get("name").unwrap_or(&Value::Null));
    format!("{prefix}{ty}-{name_part}{suffix}")
}

/// Render a scalar value as its string form for CSS output.
fn scalar_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn css_value(token: &Value) -> String {
    match token.get("type").and_then(|v| v.as_str()) {
        Some("color") => token
            .get("hex")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        Some("spacing") => {
            let obj = token.get("value").and_then(|v| v.as_object());
            match obj {
                Some(o) => o
                    .iter()
                    .map(|(k, v)| format!("{k}:{}px", scalar_str(v)))
                    .collect::<Vec<_>>()
                    .join(" "),
                None => String::new(),
            }
        }
        Some("typography") => {
            let obj = token.get("value").and_then(|v| v.as_object());
            match obj {
                Some(o) => o
                    .iter()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, v)| format!("{k}:{}", scalar_str(v)))
                    .collect::<Vec<_>>()
                    .join(" "),
                None => String::new(),
            }
        }
        Some("shadow") => {
            let value = token.get("value").cloned().unwrap_or(Value::Null);
            let color = value.get("color").cloned().unwrap_or(Value::Null);
            let offset = value.get("offset").cloned().unwrap_or(Value::Null);
            let x = offset.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = offset.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let radius = value.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let spread = value.get("spread").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let r = clamp_channel(color.get("r").and_then(|v| v.as_f64()));
            let g = clamp_channel(color.get("g").and_then(|v| v.as_f64()));
            let b = clamp_channel(color.get("b").and_then(|v| v.as_f64()));
            let a = color.get("a").and_then(|v| v.as_f64()).unwrap_or(1.0);
            format!(
                "{}px {}px {}px {}px rgba({r}, {g}, {b}, {})",
                num_str(x),
                num_str(y),
                num_str(radius),
                num_str(spread),
                num_str(a)
            )
        }
        _ => {
            // (pr-str (:value token)) — EDN-ish; approximate with JSON.
            token
                .get("value")
                .map(|v| v.to_string())
                .unwrap_or_default()
        }
    }
}

/// Format a float without a trailing `.0` for integers.
fn num_str(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Render tokens as CSS custom properties.
pub fn tokens_to_css(tokens: &[Value], prefix: &str) -> String {
    let lines: Vec<String> = tokens
        .iter()
        .map(|t| format!("  {}: {};", token_key(prefix, t, ""), css_value(t)))
        .collect();
    format!(":root {{\n{}\n}}", lines.join("\n"))
}

/// Render tokens as SCSS variables.
pub fn tokens_to_scss(tokens: &[Value], prefix: &str) -> String {
    tokens
        .iter()
        .map(|t| format!("{}: {};", token_key(prefix, t, ""), css_value(t)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render tokens as a Tailwind theme extension object.
pub fn tokens_to_tailwind(tokens: &[Value]) -> Value {
    let mut colors = Map::new();
    let non_alnum = regex::Regex::new(r"[^a-z0-9]+").unwrap();
    for t in tokens {
        if t.get("type").and_then(|v| v.as_str()) == Some("color") {
            let raw = t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let key = non_alnum.replace_all(&raw, "-").into_owned();
            let hex = t.get("hex").cloned().unwrap_or(Value::Null);
            colors.insert(key, hex);
        }
    }
    json!({"theme": {"extend": {"colors": Value::Object(colors)}}})
}

/// Formatted tokens output: a JSON `Value` for `json`/`tailwind`, a string for
/// `css`/`scss`.
pub enum Formatted {
    Text(String),
    Json(Value),
}

/// Format tokens according to `format` (json | css | scss | tailwind).
pub fn format_tokens(tokens: &[Value], format: &str, prefix: &str) -> Formatted {
    match format {
        "css" => Formatted::Text(tokens_to_css(tokens, prefix)),
        "scss" => {
            let scss_prefix = if prefix == "--figma-" {
                "$figma-"
            } else {
                prefix
            };
            Formatted::Text(tokens_to_scss(tokens, scss_prefix))
        }
        "tailwind" => Formatted::Json(tokens_to_tailwind(tokens)),
        _ => Formatted::Json(Value::Array(tokens.to_vec())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::compact;

    fn sample_tree() -> Value {
        json!({
            "id": "0:1", "name": "Page", "type": "PAGE",
            "children": [
                {"id": "1:1", "name": "Primary", "type": "FRAME",
                 "fills": [{"type": "SOLID", "color": {"r": 0.2, "g": 0.4, "b": 0.8, "a": 1}}],
                 "layout": {"itemSpacing": 8, "paddingLeft": 16},
                 "children": [{"id": "1:2", "name": "Title", "type": "TEXT",
                               "characters": "Hello", "style": {"fontFamily": "Inter", "fontSize": 24}}]},
                {"id": "1:3", "name": "Card", "type": "FRAME",
                 "fills": [{"type": "SOLID", "color": {"r": 1, "g": 1, "b": 1}}],
                 "effects": [{"type": "DROP_SHADOW", "color": {"r": 0, "g": 0, "b": 0, "a": 0.1},
                              "offset": {"x": 0, "y": 2}, "radius": 4, "spread": 0}]}
            ]
        })
    }

    #[test]
    fn extracts_colors() {
        let result = extract_tokens(&sample_tree());
        let colors: Vec<&Value> = result.iter().filter(|t| t["type"] == "color").collect();
        assert_eq!(colors.len(), 2);
        assert_eq!(colors[0]["name"], "Primary");
        assert_eq!(colors[0]["hex"], "#3366cc");
    }

    #[test]
    fn extracts_typography() {
        let result = extract_tokens(&sample_tree());
        let typos: Vec<&Value> = result
            .iter()
            .filter(|t| t["type"] == "typography")
            .collect();
        assert_eq!(typos.len(), 1);
        assert_eq!(typos[0]["name"], "Title");
    }

    #[test]
    fn extracts_typography_from_compacted() {
        let simplified = compact::simplify_tree(&sample_tree(), Some(3));
        let result = extract_tokens(&simplified);
        let typos: Vec<&Value> = result
            .iter()
            .filter(|t| t["type"] == "typography")
            .collect();
        assert_eq!(typos.len(), 1);
        assert_eq!(typos[0]["value"]["fontSize"], 24);
    }

    #[test]
    fn extracts_spacing() {
        let result = extract_tokens(&sample_tree());
        let spacings = result.iter().filter(|t| t["type"] == "spacing").count();
        assert!(spacings > 0);
    }

    #[test]
    fn extracts_effects() {
        let result = extract_tokens(&sample_tree());
        let shadows: Vec<&Value> = result.iter().filter(|t| t["type"] == "shadow").collect();
        assert_eq!(shadows.len(), 1);
        assert_eq!(shadows[0]["name"], "Card");
    }

    #[test]
    fn groups_by_category() {
        let tokens = extract_tokens(&sample_tree());
        let grouped = tokens_by_category(&tokens);
        for key in ["color", "typography", "spacing", "shadow"] {
            assert!(grouped.get(key).is_some(), "missing {key}");
        }
    }

    #[test]
    fn formats_css() {
        let tokens = extract_tokens(&sample_tree());
        let css = match format_tokens(&tokens, "css", "--figma-") {
            Formatted::Text(s) => s,
            _ => panic!("expected text"),
        };
        assert!(css.contains(":root"));
        assert!(css.contains("--figma-color-primary: #3366cc;"));
    }

    #[test]
    fn formats_tailwind() {
        let tokens = extract_tokens(&sample_tree());
        let tw = match format_tokens(&tokens, "tailwind", "--figma-") {
            Formatted::Json(v) => v,
            _ => panic!("expected json"),
        };
        assert_eq!(tw["theme"]["extend"]["colors"]["primary"], "#3366cc");
    }
}
