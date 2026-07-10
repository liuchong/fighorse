//! Unified HTTP client for the Figma REST API.
//!
//! Unified HTTP client: JSON in/out, a configurable timeout via
//! `FIGHORSE_HTTP_TIMEOUT_MS`, and error responses surfaced with status + parsed
//! body. All requests target `https://api.figma.com`.

use crate::error::{Error, Result};
use crate::url as figma_url;
use reqwest::{Client, Method};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

const BASE_URL: &str = "https://api.figma.com";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Base URL for requests. Honors `FIGHORSE_API_BASE_URL` (used by tests) but
/// defaults to the production Figma API host.
fn base_url() -> String {
    std::env::var("FIGHORSE_API_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| BASE_URL.to_string())
}

/// Resolve the request timeout, honoring a positive `FIGHORSE_HTTP_TIMEOUT_MS`.
pub fn request_timeout_ms() -> u64 {
    if let Ok(raw) = std::env::var("FIGHORSE_HTTP_TIMEOUT_MS") {
        if let Ok(parsed) = raw.trim().parse::<u64>() {
            if parsed > 0 {
                return parsed;
            }
        }
    }
    DEFAULT_TIMEOUT_MS
}

/// Percent-encode a value so it cannot inject extra URL path segments.
pub fn path_segment(value: &str) -> String {
    figma_url::encode_uri_component(value)
}

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        // reqwest reads HTTP(S)_PROXY from the environment by default; honor the
        // same variables config::setup_proxy sets. Per-request timeouts are
        // applied at each call site.
        Client::builder().build().unwrap_or_else(|_| Client::new())
    })
}

fn method_from(method: &str) -> Method {
    match method {
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        _ => Method::GET,
    }
}

fn parse_body(text: &str) -> Value {
    if text.trim().is_empty() {
        return Value::Object(serde_json::Map::new());
    }
    match serde_json::from_str::<Value>(text) {
        Ok(v) => v,
        Err(_) => {
            let mut m = serde_json::Map::new();
            m.insert("raw".into(), Value::String(text.to_string()));
            Value::Object(m)
        }
    }
}

/// Make an HTTP request to the Figma API.
///
/// - `method`: "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
/// - `path`: API path (e.g. `/v1/files/abc`)
/// - `token`: Figma Personal Access Token (empty/None omits the header)
/// - `params`: ordered query parameters (Null values omitted)
/// - `body`: optional JSON request body
pub async fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    params: &[(&str, Value)],
    body: Option<&Value>,
) -> Result<Value> {
    let full = format!("{}{path}", base_url());
    let url = figma_url::build_url(&full, params);

    let mut req = client()
        .request(method_from(method), &url)
        .header("Content-Type", "application/json")
        .timeout(Duration::from_millis(request_timeout_ms()));

    if let Some(t) = token {
        if !t.is_empty() {
            req = req.header("X-Figma-Token", t);
        }
    }

    if let Some(b) = body {
        req = req.body(serde_json::to_string(b)?);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() => return Err(Error::Timeout(request_timeout_ms())),
        Err(e) => return Err(Error::from(e)),
    };

    let status = resp.status();
    let status_text = status
        .canonical_reason()
        .unwrap_or("")
        .to_string();
    let text = resp.text().await.unwrap_or_default();
    let parsed = parse_body(&text);

    if status.is_success() {
        Ok(parsed)
    } else {
        Err(Error::Figma {
            status: status.as_u16(),
            status_text,
            body: parsed,
        })
    }
}

/// GET request with optional query params.
pub async fn get(path: &str, token: Option<&str>, params: &[(&str, Value)]) -> Result<Value> {
    request("GET", path, token, params, None).await
}

/// POST request with optional params and JSON body.
pub async fn post(
    path: &str,
    token: Option<&str>,
    params: &[(&str, Value)],
    body: Option<&Value>,
) -> Result<Value> {
    request("POST", path, token, params, body).await
}

/// PUT request with optional params and JSON body.
pub async fn put(
    path: &str,
    token: Option<&str>,
    params: &[(&str, Value)],
    body: Option<&Value>,
) -> Result<Value> {
    request("PUT", path, token, params, body).await
}

/// PATCH request with optional params and JSON body.
pub async fn patch(
    path: &str,
    token: Option<&str>,
    params: &[(&str, Value)],
    body: Option<&Value>,
) -> Result<Value> {
    request("PATCH", path, token, params, body).await
}

/// DELETE request with optional query params.
pub async fn delete(path: &str, token: Option<&str>, params: &[(&str, Value)]) -> Result<Value> {
    request("DELETE", path, token, params, None).await
}

/// Perform a raw GET to an arbitrary URL (e.g. downloading rendered images from
/// Figma's CDN), returning the reqwest response for streaming.
pub async fn raw_get(url: &str) -> Result<reqwest::Response> {
    let resp = client()
        .get(url)
        .timeout(Duration::from_millis(request_timeout_ms()))
        .send()
        .await;
    match resp {
        Ok(r) => Ok(r),
        Err(e) if e.is_timeout() => Err(Error::Timeout(request_timeout_ms())),
        Err(e) => Err(Error::from(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_env_override() {
        std::env::set_var("FIGHORSE_HTTP_TIMEOUT_MS", "42");
        assert_eq!(request_timeout_ms(), 42);
        std::env::set_var("FIGHORSE_HTTP_TIMEOUT_MS", "0");
        assert_eq!(request_timeout_ms(), 120_000);
        std::env::remove_var("FIGHORSE_HTTP_TIMEOUT_MS");
    }

    #[test]
    fn path_segment_encodes() {
        assert_eq!(path_segment("a/b?c=d"), "a%2Fb%3Fc%3Dd");
    }
}
