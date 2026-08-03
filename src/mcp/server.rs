//! MCP server lifecycle, singleton lock, and official rmcp transports.

use crate::config;
use crate::error::{Error, Result};
use crate::mcp::{handler::FighorseHandler, resources, tools};
use rmcp::{
    ServiceExt,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

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
                unsafe extern "C" {
                    fn kill(pid: i32, sig: i32) -> i32;
                }
                kill(p as i32, 0) == 0 || std::io::Error::last_os_error().raw_os_error() != Some(3)
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
                let existing_pid = existing
                    .as_ref()
                    .and_then(|l| l.get("pid"))
                    .and_then(|v| v.as_i64());
                if active_pid(existing_pid) {
                    let pid_note = existing_pid
                        .map(|p| format!(" (pid {p})"))
                        .unwrap_or_default();
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

impl Drop for Lock {
    fn drop(&mut self) {
        release_singleton_lock(self);
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
        "serverInfo": {"name": "fighorse", "version": env!("CARGO_PKG_VERSION")},
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
        "initialize" => Some(jsonrpc_result(
            id.unwrap_or(Value::Null),
            initialize_result(&params),
        )),
        "ping" => Some(jsonrpc_result(id.unwrap_or(Value::Null), json!({}))),
        "tools/list" => Some(jsonrpc_result(
            id.unwrap_or(Value::Null),
            tools::list_tools(),
        )),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let result = tools::call_tool(name, &args).await;
            Some(jsonrpc_result(id.unwrap_or(Value::Null), result))
        }
        "resources/list" => Some(jsonrpc_result(
            id.unwrap_or(Value::Null),
            resources::list_resources(),
        )),
        "resources/read" => {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            match resources::read_resource(uri) {
                Ok(v) => Some(jsonrpc_result(id.unwrap_or(Value::Null), v)),
                Err(e) => Some(jsonrpc_error(
                    id.unwrap_or(Value::Null),
                    -32603,
                    &e.to_string(),
                )),
            }
        }
        "prompts/list" => Some(jsonrpc_result(
            id.unwrap_or(Value::Null),
            resources::list_prompts(),
        )),
        "prompts/get" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            match resources::get_prompt(name, &arguments) {
                Ok(v) => Some(jsonrpc_result(id.unwrap_or(Value::Null), v)),
                Err(e) => Some(jsonrpc_error(
                    id.unwrap_or(Value::Null),
                    -32603,
                    &e.to_string(),
                )),
            }
        }
        "notifications/initialized" | "notifications/cancelled" => None,
        _ => id.map(|id| jsonrpc_error(id, -32601, &format!("Method not found: {method}"))),
    }
}

// --- stdio transport ---

/// Run the stdio transport until stdin closes.
pub async fn serve_stdio() -> Result<()> {
    if std::env::var("FIGHORSE_MCP_STDIO_LOG").as_deref() == Ok("1") {
        eprintln!("Fighorse MCP server started on stdio");
    }
    let service = FighorseHandler
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| Error::Other(format!("MCP stdio startup error: {error}")))?;
    service
        .waiting()
        .await
        .map_err(|error| Error::Other(format!("MCP stdio transport error: {error}")))?;
    Ok(())
}

/// Start the MCP server on the given transport with singleton lock + signal
/// handling.
pub async fn serve(
    transport: &str,
    port: i64,
    host: Option<&str>,
    cors_origin: Option<&str>,
) -> Result<()> {
    if transport == "sse" {
        return Err(Error::Other(
            "Legacy SSE transport is retired. Use `fighorse mcp serve --transport http` and \
             configure clients with http://127.0.0.1:9449/mcp."
                .into(),
        ));
    }
    if !matches!(transport, "stdio" | "http") {
        return Err(Error::Other(format!(
            "Unknown MCP transport: {transport}. Expected http or stdio."
        )));
    }

    let _lock = acquire_singleton_lock(transport, port)?;
    match transport {
        "stdio" => {
            tokio::select! {
                result = serve_stdio() => result,
                _ = shutdown_signal() => Ok(()),
            }
        }
        "http" => serve_http(port, host, cors_origin).await,
        _ => unreachable!(),
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = async {
                if let Some(signal) = sigterm.as_mut() {
                    signal.recv().await;
                }
            } => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

// --- Streamable HTTP transport ---

#[derive(Clone)]
struct HttpRequestPolicy {
    allowed_hosts: Vec<String>,
    allowed_origins: Vec<String>,
}

async fn enforce_http_request_policy(
    axum::extract::State(policy): axum::extract::State<HttpRequestPolicy>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::{http::StatusCode, response::IntoResponse};

    let host_allowed = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| {
            policy
                .allowed_hosts
                .iter()
                .any(|allowed| host.eq_ignore_ascii_case(allowed))
        });
    if !host_allowed {
        return StatusCode::FORBIDDEN.into_response();
    }

    if let Some(origin) = request
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        if !policy
            .allowed_origins
            .iter()
            .any(|allowed| origin.eq_ignore_ascii_case(allowed))
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    next.run(request).await
}

async fn normalize_successful_session_delete(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{Method, StatusCode};

    let is_session_delete = request.method() == Method::DELETE && request.uri().path() == "/mcp";
    let mut response = next.run(request).await;
    if is_session_delete && response.status() == StatusCode::ACCEPTED {
        *response.status_mut() = StatusCode::OK;
    }
    response
}

async fn serve_http(port: i64, host: Option<&str>, cors_origin: Option<&str>) -> Result<()> {
    use axum::{Json, Router, middleware, response::IntoResponse, routing::get};

    let host = host.unwrap_or("127.0.0.1").to_string();

    async fn manifest_handler() -> impl IntoResponse {
        Json(crate::discovery::manifest())
    }
    async fn health_handler() -> impl IntoResponse {
        Json(crate::discovery::doctor())
    }

    let cancellation_token = CancellationToken::new();
    let mut allowed_origins = vec![
        format!("http://127.0.0.1:{port}"),
        format!("http://localhost:{port}"),
    ];
    if let Some(configured) = cors_origin {
        allowed_origins.extend(
            configured
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned),
        );
    }
    let allowed_hosts = vec![
        host.clone(),
        format!("{host}:{port}"),
        "localhost".to_string(),
        format!("localhost:{port}"),
    ];
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(allowed_hosts.clone())
        .with_allowed_origins(allowed_origins.clone())
        .with_cancellation_token(cancellation_token.child_token());
    let mcp: StreamableHttpService<FighorseHandler, LocalSessionManager> =
        StreamableHttpService::new(|| Ok(FighorseHandler), Default::default(), config);

    let app = Router::new()
        .route("/", get(manifest_handler))
        .route("/manifest", get(manifest_handler))
        .route("/discover", get(manifest_handler))
        .route("/health", get(health_handler))
        .nest_service("/mcp", mcp)
        .layer(middleware::from_fn_with_state(
            HttpRequestPolicy {
                allowed_hosts,
                allowed_origins,
            },
            enforce_http_request_policy,
        ))
        .layer(middleware::from_fn(normalize_successful_session_delete));

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| Error::Other(format!("Fighorse MCP server error: {e}")))?;
    eprintln!("Fighorse MCP server listening on http://{host}:{port}/mcp");
    let shutdown_token = cancellation_token.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            shutdown_token.cancel();
        })
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
        unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "readonly") };
        let msg = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
        let resp = dispatch(&msg).await.unwrap();
        assert!(resp["result"]["tools"].as_array().unwrap().len() > 10);
        unsafe { std::env::remove_var("FIGHORSE_MCP_MODE") };
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
}
