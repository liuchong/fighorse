//! MCP server: singleton lock, lifecycle, and stdio + streamable HTTP/SSE
//! transports.
//!
//! The JSON-RPC dispatch reuses the
//! transport-agnostic `tools`/`resources` core. stdio framing supports both
//! newline-delimited JSON and `Content-Length` headers via the dual
//! dual-mode transport.

use crate::config;
use crate::error::{Error, Result};
use crate::mcp::{resources, tools};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};

const DEFAULT_STDIO_MAX_BYTES: usize = 10_485_760;
const PROTOCOL_VERSION: &str = "2025-11-25";

fn env_flag(key: &str) -> bool {
    std::env::var(key)
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true"))
        .unwrap_or(false)
}

fn mcp_multiple_enabled() -> bool {
    env_flag("FIGHORSE_MCP_ALLOW_MULTIPLE")
}

fn mcp_lock_file() -> PathBuf {
    if let Ok(f) = std::env::var("FIGHORSE_MCP_LOCK_FILE") {
        if !f.is_empty() {
            return PathBuf::from(f);
        }
    }
    config::fighorse_home().join("runtime").join("mcp.lock")
}

fn active_pid(pid: Option<i64>) -> bool {
    match pid {
        Some(p) if p > 0 => {
            #[cfg(unix)]
            unsafe {
                extern "C" {
                    fn kill(pid: i32, sig: i32) -> i32;
                }
                kill(p as i32, 0) == 0
                    || std::io::Error::last_os_error().raw_os_error() != Some(3)
            }
            #[cfg(not(unix))]
            {
                false
            }
        }
        _ => false,
    }
}

fn read_lock_file(file: &PathBuf) -> Option<Value> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
}

/// A held singleton lock.
pub struct Lock {
    file: PathBuf,
    pid: u32,
}

/// Acquire the process-wide MCP server lock, clearing stale locks.
pub fn acquire_singleton_lock(transport: &str, port: i64) -> Result<Option<Lock>> {
    if mcp_multiple_enabled() {
        return Ok(None);
    }
    let file = mcp_lock_file();
    let pid = std::process::id();

    for attempt in 0..2 {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Exclusive create ("wx").
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
        {
            Ok(mut f) => {
                let data = json!({
                    "kind": "fighorse.mcp-lock.v1",
                    "pid": pid,
                    "transport": transport,
                    "port": port,
                });
                use std::io::Write;
                f.write_all(serde_json::to_string_pretty(&data)?.as_bytes())?;
                return Ok(Some(Lock { file, pid }));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempt == 0 => {
                let existing = read_lock_file(&file);
                let existing_pid = existing.as_ref().and_then(|l| l.get("pid")).and_then(|v| v.as_i64());
                if active_pid(existing_pid) {
                    let pid_note = existing_pid.map(|p| format!(" (pid {p})")).unwrap_or_default();
                    return Err(Error::Other(format!(
                        "Another fighorse MCP server is already running{pid_note}. Stop it first or set FIGHORSE_MCP_ALLOW_MULTIPLE=1 for development."
                    )));
                }
                // Stale lock: remove and retry.
                let _ = std::fs::remove_file(&file);
            }
            Err(e) => return Err(Error::from(e)),
        }
    }
    Err(Error::Other("Failed to acquire MCP singleton lock".into()))
}

/// Release the lock if we still own it.
pub fn release_singleton_lock(lock: &Lock) {
    if let Some(existing) = read_lock_file(&lock.file) {
        if existing.get("pid").and_then(|v| v.as_i64()) == Some(lock.pid as i64) {
            let _ = std::fs::remove_file(&lock.file);
        }
    }
}

// --- JSON-RPC dispatch (shared across transports) ---

fn jsonrpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn jsonrpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn initialize_result(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": requested,
        "capabilities": {"tools": {}, "resources": {}, "prompts": {}},
        "serverInfo": {"name": "fighorse", "version": "0.1.0"},
        "instructions": "Call discover_fighorse first. For Figma replication, ask when platform or asset_format is missing, export assets with manifests, and record reusable lessons after visual fixes."
    })
}

/// Dispatch a single JSON-RPC request, returning an optional response value
/// (notifications return None).
pub async fn dispatch(message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(json!({}));

    match method {
        "initialize" => Some(jsonrpc_result(id.unwrap_or(Value::Null), initialize_result(&params))),
        "ping" => Some(jsonrpc_result(id.unwrap_or(Value::Null), json!({}))),
        "tools/list" => Some(jsonrpc_result(id.unwrap_or(Value::Null), tools::list_tools())),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let result = tools::call_tool(name, &args).await;
            Some(jsonrpc_result(id.unwrap_or(Value::Null), result))
        }
        "resources/list" => Some(jsonrpc_result(id.unwrap_or(Value::Null), resources::list_resources())),
        "resources/read" => {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            match resources::read_resource(uri) {
                Ok(v) => Some(jsonrpc_result(id.unwrap_or(Value::Null), v)),
                Err(e) => Some(jsonrpc_error(id.unwrap_or(Value::Null), -32603, &e.to_string())),
            }
        }
        "prompts/list" => Some(jsonrpc_result(id.unwrap_or(Value::Null), resources::list_prompts())),
        "prompts/get" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            match resources::get_prompt(name, &arguments) {
                Ok(v) => Some(jsonrpc_result(id.unwrap_or(Value::Null), v)),
                Err(e) => Some(jsonrpc_error(id.unwrap_or(Value::Null), -32603, &e.to_string())),
            }
        }
        "notifications/initialized" | "notifications/cancelled" => None,
        _ => id.map(|id| jsonrpc_error(id, -32601, &format!("Method not found: {method}"))),
    }
}

// --- stdio transport ---

fn stdio_max_bytes() -> usize {
    std::env::var("FIGHORSE_MCP_STDIO_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_STDIO_MAX_BYTES)
}

/// Serialize an outgoing message. `header` mode uses Content-Length framing;
/// otherwise newline-delimited JSON.
fn serialize_message(message: &Value, header_mode: bool) -> Vec<u8> {
    let json = serde_json::to_string(message).unwrap_or_else(|_| "{}".into());
    if header_mode {
        format!("Content-Length: {}\r\n\r\n{json}", json.as_bytes().len()).into_bytes()
    } else {
        format!("{json}\n").into_bytes()
    }
}

/// Run the stdio transport until stdin closes.
///
/// Reads a byte stream and frames messages, auto-detecting Content-Length vs
/// newline mode from the first message. The
/// reply mode follows the detected request mode.
pub async fn serve_stdio() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();
    let mut buffer: Vec<u8> = Vec::new();
    let mut header_mode = false;
    let mut mode_locked = false;
    let max_bytes = stdio_max_bytes();

    if std::env::var("FIGHORSE_MCP_STDIO_LOG").as_deref() == Ok("1") {
        eprintln!("Fighorse MCP server started on stdio");
    }

    let mut chunk = [0u8; 65536];
    loop {
        let n = {
            use tokio::io::AsyncReadExt;
            reader.read(&mut chunk).await?
        };
        if n == 0 {
            break; // stdin closed
        }
        buffer.extend_from_slice(&chunk[..n]);

        loop {
            match extract_message(&buffer, &mut header_mode, &mut mode_locked, max_bytes) {
                Ok(Some((msg_bytes, consumed))) => {
                    buffer.drain(..consumed);
                    if msg_bytes.is_empty() {
                        continue; // blank line
                    }
                    let parsed: Value = match serde_json::from_slice(&msg_bytes) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("MCP stdio parse error: {e}");
                            continue;
                        }
                    };
                    if let Some(response) = dispatch(&parsed).await {
                        stdout.write_all(&serialize_message(&response, header_mode)).await?;
                        stdout.flush().await?;
                    }
                }
                Ok(None) => break, // need more data
                Err(e) => {
                    eprintln!("MCP stdio error: {e}");
                    buffer.clear();
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Try to extract one framed message from the buffer. Returns the message bytes
/// (empty for a blank line) and the number of bytes consumed, or None if more
/// data is needed.
fn extract_message(
    buffer: &[u8],
    header_mode: &mut bool,
    mode_locked: &mut bool,
    max_bytes: usize,
) -> Result<Option<(Vec<u8>, usize)>> {
    if buffer.is_empty() {
        return Ok(None);
    }

    // Detect header separator.
    let crlf = find_subsequence(buffer, b"\r\n\r\n");
    let lf = find_subsequence(buffer, b"\n\n");
    let header_sep = match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    // Prefix (lowercased) to check for content-length.
    let prefix_len = buffer.len().min(32);
    let prefix = String::from_utf8_lossy(&buffer[..prefix_len]).to_lowercase();
    let looks_header = prefix.starts_with("content-length:");

    if *header_mode || looks_header {
        // Header framing.
        let sep = match header_sep {
            Some(s) => s,
            None => return Ok(None),
        };
        let sep_len = if crlf == Some(sep) { 4 } else { 2 };
        let header = String::from_utf8_lossy(&buffer[..sep]);
        let re = regex::Regex::new(r"(?i)content-length:\s*(\d+)").unwrap();
        let content_length: usize = match re.captures(&header) {
            Some(c) => c[1].parse().unwrap_or(0),
            None => return Err(Error::Other("Missing Content-Length header in MCP stdio message".into())),
        };
        if content_length > max_bytes {
            return Err(Error::Other(format!(
                "MCP stdio message exceeds max size: {content_length}"
            )));
        }
        let body_start = sep + sep_len;
        let body_end = body_start + content_length;
        if buffer.len() < body_end {
            return Ok(None);
        }
        *header_mode = true;
        *mode_locked = true;
        return Ok(Some((buffer[body_start..body_end].to_vec(), body_end)));
    }

    // Newline framing.
    if let Some(idx) = find_subsequence(buffer, b"\n") {
        let mut line = &buffer[..idx];
        if line.last() == Some(&b'\r') {
            line = &line[..line.len() - 1];
        }
        *mode_locked = true;
        return Ok(Some((line.to_vec(), idx + 1)));
    }

    Ok(None)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Start the MCP server on the given transport with singleton lock + signal
/// handling.
pub async fn serve(transport: &str, port: i64, host: Option<&str>, cors_origin: Option<&str>) -> Result<()> {
    let lock = match acquire_singleton_lock(transport, port) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let lock = Arc::new(lock);

    // Signal handling: release lock on SIGINT/SIGTERM.
    {
        let lock = lock.clone();
        tokio::spawn(async move {
            let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).ok();
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
            let wait_int = async {
                if let Some(s) = sigint.as_mut() {
                    s.recv().await;
                }
            };
            let wait_term = async {
                if let Some(s) = sigterm.as_mut() {
                    s.recv().await;
                }
            };
            tokio::select! {
                _ = wait_int => {}
                _ = wait_term => {}
            }
            if let Some(l) = lock.as_ref() {
                release_singleton_lock(l);
            }
            std::process::exit(0);
        });
    }

    let result = match transport {
        "stdio" => serve_stdio().await,
        "sse" | "http" => serve_http(port, host, cors_origin).await,
        other => {
            eprintln!("Unknown MCP transport: {other}");
            if let Some(l) = lock.as_ref() {
                release_singleton_lock(l);
            }
            std::process::exit(1);
        }
    };

    if let Some(l) = lock.as_ref() {
        release_singleton_lock(l);
    }
    result
}

// --- HTTP / SSE transport ---

async fn serve_http(port: i64, host: Option<&str>, _cors_origin: Option<&str>) -> Result<()> {
    use axum::{
        extract::Query,
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use std::collections::HashMap;

    let host = host.unwrap_or("127.0.0.1").to_string();

    async fn manifest_handler() -> impl IntoResponse {
        Json(crate::discovery::manifest())
    }
    async fn health_handler() -> impl IntoResponse {
        Json(crate::discovery::doctor())
    }
    async fn mcp_post(Json(body): Json<Value>) -> impl IntoResponse {
        // Stateless streamable HTTP: dispatch one JSON-RPC message per request.
        match dispatch(&body).await {
            Some(response) => (StatusCode::OK, Json(response)).into_response(),
            None => (StatusCode::ACCEPTED, Json(json!({}))).into_response(),
        }
    }
    async fn mcp_get(Query(_q): Query<HashMap<String, String>>) -> impl IntoResponse {
        (StatusCode::OK, Json(crate::discovery::manifest())).into_response()
    }

    let app = Router::new()
        .route("/", get(manifest_handler))
        .route("/manifest", get(manifest_handler))
        .route("/discover", get(manifest_handler))
        .route("/health", get(health_handler))
        .route("/mcp", post(mcp_post).get(mcp_get));

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| Error::Other(format!("Fighorse MCP server error: {e}")))?;
    eprintln!("Fighorse MCP server listening on http://{host}:{port}/mcp");
    axum::serve(listener, app)
        .await
        .map_err(|e| Error::Other(format!("Fighorse MCP server error: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dispatch_initialize() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}});
        let resp = dispatch(&msg).await.unwrap();
        assert_eq!(resp["result"]["serverInfo"]["name"], "fighorse");
        assert_eq!(resp["result"]["protocolVersion"], "2025-06-18");
    }

    #[tokio::test]
    async fn dispatch_tools_list() {
        std::env::set_var("FIGHORSE_MCP_MODE", "readonly");
        let msg = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
        let resp = dispatch(&msg).await.unwrap();
        assert!(resp["result"]["tools"].as_array().unwrap().len() > 10);
        std::env::remove_var("FIGHORSE_MCP_MODE");
    }

    #[tokio::test]
    async fn dispatch_notification_returns_none() {
        let msg = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(dispatch(&msg).await.is_none());
    }

    #[tokio::test]
    async fn dispatch_parse_figma_url_tool() {
        let msg = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "parse_figma_url", "arguments": {"figma_url": "https://figma.com/design/abc/T?node-id=1-2"}}});
        let resp = dispatch(&msg).await.unwrap();
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"file_key\": \"abc\""));
        assert!(text.contains("\"node_id\": \"1:2\""));
    }

    #[test]
    fn extract_newline_message() {
        let mut hm = false;
        let mut locked = false;
        let buf = b"{\"a\":1}\n".to_vec();
        let (msg, consumed) = extract_message(&buf, &mut hm, &mut locked, 1000).unwrap().unwrap();
        assert_eq!(msg, b"{\"a\":1}");
        assert_eq!(consumed, 8);
        assert!(!hm);
    }

    #[test]
    fn extract_header_message() {
        let mut hm = false;
        let mut locked = false;
        let buf = b"Content-Length: 7\r\n\r\n{\"a\":1}".to_vec();
        let (msg, _consumed) = extract_message(&buf, &mut hm, &mut locked, 1000).unwrap().unwrap();
        assert_eq!(msg, b"{\"a\":1}");
        assert!(hm);
    }
}
