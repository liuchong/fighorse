//! URL and query string utilities.
//!
//! URL and query-string utilities. Query values are percent-encoded the same way
//! JavaScript's `encodeURIComponent` encodes them, so output matches the
//! JavaScript's `encodeURIComponent` encodes them.

use serde_json::Value;

/// Percent-encode a string exactly like JavaScript's `encodeURIComponent`.
///
/// Unreserved set (left as-is): `A-Za-z0-9` and `- _ . ! ~ * ' ( )`.
pub fn encode_uri_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            );
        if keep {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

/// Render a query-string scalar as its string form (JS `String(v)` semantics).
pub fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

/// Build a URL query string from ordered key/value pairs. `None`/`Null` values
/// are omitted. Returns `None` when nothing remains.
pub fn build_query(params: &[(&str, Value)]) -> Option<String> {
    let parts: Vec<String> = params
        .iter()
        .filter_map(|(k, v)| {
            value_to_string(v).map(|s| format!("{k}={}", encode_uri_component(&s)))
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(format!("?{}", parts.join("&")))
    }
}

/// Build a full URL with optional query params.
pub fn build_url(base_path: &str, params: &[(&str, Value)]) -> String {
    match build_query(params) {
        Some(q) => format!("{base_path}{q}"),
        None => base_path.to_string(),
    }
}

/// Convert a Figma URL node-id (`1-2`) to REST API form (`1:2`).
pub fn normalize_node_id(node_id: &str) -> String {
    node_id.replace('-', ":")
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Result of parsing a Figma URL or raw file key.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedUrl {
    pub valid: bool,
    pub input: String,
    pub file_key: Option<String>,
    pub node_id: Option<String>,
    pub raw_node_id: Option<String>,
    pub kind: Option<String>,
    pub embedded_url: Option<String>,
    pub error: Option<String>,
}

impl ParsedUrl {
    fn invalid(input: &str, error: &str) -> Self {
        ParsedUrl {
            valid: false,
            input: input.to_string(),
            file_key: None,
            node_id: None,
            raw_node_id: None,
            kind: None,
            embedded_url: None,
            error: Some(error.to_string()),
        }
    }

    fn invalid_with_kind(input: &str, kind: Option<String>, error: &str) -> Self {
        ParsedUrl {
            valid: false,
            input: input.to_string(),
            file_key: None,
            node_id: None,
            raw_node_id: None,
            kind,
            embedded_url: None,
            error: Some(error.to_string()),
        }
    }

    /// Serialize to the parsed-URL JSON shape.
    pub fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("valid".into(), Value::Bool(self.valid));
        map.insert("input".into(), Value::String(self.input.clone()));
        if let Some(k) = &self.kind {
            map.insert("kind".into(), Value::String(k.clone()));
        }
        if let Some(fk) = &self.file_key {
            map.insert("file_key".into(), Value::String(fk.clone()));
        }
        if let Some(raw) = &self.raw_node_id {
            map.insert("raw_node_id".into(), Value::String(raw.clone()));
        }
        if let Some(n) = &self.node_id {
            map.insert("node_id".into(), Value::String(n.clone()));
        }
        if let Some(e) = &self.embedded_url {
            map.insert("embedded_url".into(), Value::String(e.clone()));
        }
        if let Some(err) = &self.error {
            map.insert("error".into(), Value::String(err.clone()));
        }
        Value::Object(map)
    }
}

fn file_key_from_segments(segments: &[String]) -> Option<String> {
    for pair in segments.windows(2) {
        let kind = &pair[0];
        if matches!(kind.as_str(), "design" | "file" | "proto" | "board") {
            return Some(pair[1].clone());
        }
    }
    None
}

fn is_figma_host(url: &url::Url) -> bool {
    url.host_str()
        .map(|host| host == "figma.com" || host.ends_with(".figma.com"))
        .unwrap_or(false)
}

fn figma_browser_url_error(kind: Option<&str>) -> Option<&'static str> {
    match kind {
        Some("files") => Some(
            "This is a Figma file browser or project URL. It does not include a design file key or selected frame node. Open the concrete Figma file and copy a link to the selected frame, component, or group, or pass a raw file key together with --node-id.",
        ),
        _ => None,
    }
}

/// Parse a Figma URL or raw file key into its components.
pub fn parse_figma_url(input: &str) -> ParsedUrl {
    let raw = input.trim().to_string();

    if raw.is_empty() {
        return ParsedUrl::invalid(&raw, "Empty Figma URL or file key");
    }

    if !is_url(&raw) {
        return ParsedUrl {
            valid: true,
            input: raw.clone(),
            file_key: Some(raw),
            node_id: None,
            raw_node_id: None,
            kind: Some("file_key".to_string()),
            embedded_url: None,
            error: None,
        };
    }

    let parsed = match url::Url::parse(&raw) {
        Ok(u) => u,
        Err(e) => return ParsedUrl::invalid(&raw, &e.to_string()),
    };

    // Recurse into an embedded `url` query param if present.
    if let Some((_, embedded)) = parsed.query_pairs().find(|(k, _)| k == "url") {
        let embedded = embedded.to_string();
        if !embedded.is_empty() {
            let mut inner = parse_figma_url(&embedded);
            inner.input = raw.clone();
            inner.embedded_url = Some(embedded);
            return inner;
        }
    }

    let segments: Vec<String> = parsed
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .map(percent_decode)
        .collect();

    let file_key = file_key_from_segments(&segments);
    let raw_node_id = parsed
        .query_pairs()
        .find(|(k, _)| k == "node-id")
        .map(|(_, v)| v.to_string());
    let kind = segments.first().cloned();

    match file_key {
        Some(fk) => ParsedUrl {
            valid: true,
            input: raw,
            file_key: Some(fk),
            node_id: raw_node_id.as_deref().map(normalize_node_id),
            raw_node_id,
            kind,
            embedded_url: None,
            error: None,
        },
        None => {
            if is_figma_host(&parsed) {
                if let Some(error) = figma_browser_url_error(kind.as_deref()) {
                    return ParsedUrl::invalid_with_kind(&raw, kind, error);
                }
            }
            ParsedUrl::invalid(&raw, "Could not find Figma file key in URL")
        }
    }
}

/// Percent-decode a URL path segment (the `url` crate leaves `%`-escapes in the
/// path); mirrors how the JS `URL` object exposes decoded path segments only
/// for the file key comparison. Figma file keys are alphanumeric, so decoding
/// is a safe no-op for realistic keys but keeps parity for odd names.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params(pairs: Vec<(&'static str, Value)>) -> Vec<(&'static str, Value)> {
        pairs
    }

    #[test]
    fn build_query_empty() {
        assert_eq!(build_query(&[]), None);
    }

    #[test]
    fn build_query_single() {
        assert_eq!(
            build_query(&params(vec![("depth", json!(2))])),
            Some("?depth=2".to_string())
        );
    }

    #[test]
    fn build_query_multiple() {
        assert_eq!(
            build_query(&params(vec![
                ("depth", json!(2)),
                ("version", json!("123"))
            ])),
            Some("?depth=2&version=123".to_string())
        );
    }

    #[test]
    fn build_query_omits_null() {
        assert_eq!(
            build_query(&params(vec![("depth", json!(2)), ("version", Value::Null)])),
            Some("?depth=2".to_string())
        );
    }

    #[test]
    fn build_query_encodes_special() {
        assert_eq!(
            build_query(&params(vec![("ids", json!("1:2,1:3"))])),
            Some("?ids=1%3A2%2C1%3A3".to_string())
        );
    }

    #[test]
    fn build_url_without_params() {
        assert_eq!(
            build_url("https://api.figma.com/v1/files/abc", &[]),
            "https://api.figma.com/v1/files/abc"
        );
    }

    #[test]
    fn build_url_with_params() {
        assert_eq!(
            build_url(
                "https://api.figma.com/v1/files/abc",
                &params(vec![("depth", json!(2))])
            ),
            "https://api.figma.com/v1/files/abc?depth=2"
        );
    }

    #[test]
    fn parse_design_url() {
        let p = parse_figma_url(
            "https://www.figma.com/design/SApEHB4JyKN2I8PpMYfgbK/Intent?node-id=376-5412",
        );
        assert!(p.valid);
        assert_eq!(p.kind.as_deref(), Some("design"));
        assert_eq!(p.file_key.as_deref(), Some("SApEHB4JyKN2I8PpMYfgbK"));
        assert_eq!(p.raw_node_id.as_deref(), Some("376-5412"));
        assert_eq!(p.node_id.as_deref(), Some("376:5412"));
    }

    #[test]
    fn parse_raw_file_key() {
        let p = parse_figma_url("abc123");
        assert!(p.valid);
        assert_eq!(p.file_key.as_deref(), Some("abc123"));
        assert_eq!(p.kind.as_deref(), Some("file_key"));
    }

    #[test]
    fn parse_invalid_url() {
        let p = parse_figma_url("https://example.com/no-file-key");
        assert!(!p.valid);
    }

    #[test]
    fn parse_figma_file_browser_url_explains_missing_design_target() {
        let p = parse_figma_url("https://www.figma.com/files/project/123456/Mobile-App?fuid=abc");
        assert!(!p.valid);
        assert_eq!(p.kind.as_deref(), Some("files"));
        assert!(p.file_key.is_none());
        assert!(p.node_id.is_none());
        assert!(
            p.error
                .as_deref()
                .unwrap_or("")
                .contains("does not include a design file key")
        );
    }

    #[test]
    fn encode_component_matches_js() {
        assert_eq!(encode_uri_component("a/b?c=d"), "a%2Fb%3Fc%3Dd");
    }
}
