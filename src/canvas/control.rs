//! Loopback control HTTP server and plugin WebSocket endpoint.

use super::{
    CanvasError, CanvasErrorCode, CanvasManager, CanvasPluginRequest, CanvasPluginResponse,
    CanvasSession, PLUGIN_VERSION, PROTOCOL_VERSION,
};
use crate::config;
use crate::error::{Error, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

const CONTROL_HEADER: &str = "x-fighorse-canvas-control";

#[derive(Debug, Clone)]
pub struct ControlServer {
    pub manager: CanvasManager,
    pub secret: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
struct PairRequest {
    #[serde(default)]
    ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ApplyRequest {
    #[serde(default)]
    yes: bool,
    #[serde(flatten)]
    plan: super::CanvasPlan,
}

#[derive(Debug, Deserialize)]
struct SessionRequest {
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UndoRequest {
    transaction_id: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    yes: bool,
}

#[derive(Debug, Deserialize)]
struct ExecuteRequest {
    script: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    yes: bool,
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    code: String,
}

#[derive(Debug, Deserialize)]
struct PluginHello {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    session_id: Option<String>,
    plugin_version: String,
    editor_type: super::EditorType,
    #[serde(default)]
    document_name: Option<String>,
    #[serde(default)]
    current_page: Option<String>,
    #[serde(default)]
    selection_count: Option<u32>,
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OutboundEnvelope<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    id: &'a str,
    command: &'a str,
    params: &'a Value,
}

#[derive(Debug, Deserialize)]
struct InboundEnvelope {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<CanvasError>,
}

impl ControlServer {
    pub fn new(manager: CanvasManager, port: u16) -> Result<Self> {
        Ok(Self {
            manager,
            secret: ensure_control_secret()?,
            port,
        })
    }

    fn authorize(&self, headers: &HeaderMap) -> std::result::Result<(), (StatusCode, Json<Value>)> {
        let ok = headers
            .get(CONTROL_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value == self.secret)
            .unwrap_or(false);
        if ok {
            Ok(())
        } else {
            Err((
                StatusCode::UNAUTHORIZED,
                Json(canvas_error_json(CanvasError::new(
                    CanvasErrorCode::PermissionDenied,
                    "Invalid local canvas control credential.",
                ))),
            ))
        }
    }
}

pub fn control_secret_path() -> PathBuf {
    config::fighorse_home()
        .join("runtime")
        .join("canvas-control.json")
}

pub fn read_control_secret() -> Result<String> {
    let path = control_secret_path();
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    value
        .get("secret")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            Error::Usage(format!(
                "Invalid canvas control credential at {}",
                path.display()
            ))
        })
}

fn ensure_control_secret() -> Result<String> {
    if let Ok(secret) = read_control_secret() {
        if !secret.trim().is_empty() {
            return Ok(secret);
        }
    }
    let secret = format!("ctl-{}-{}", std::process::id(), random_hex());
    let path = control_secret_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = json!({
        "kind": "fighorse.canvas-control.v1",
        "secret": secret,
        "endpoint": format!("http://127.0.0.1:{}/canvas", config::load_config().canvas_port),
    });
    write_private(&path, serde_json::to_string_pretty(&data)?.as_bytes())?;
    Ok(secret)
}

#[cfg(unix)]
fn write_private(path: &PathBuf, content: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &PathBuf, content: &[u8]) -> Result<()> {
    std::fs::write(path, content)?;
    Ok(())
}

pub async fn serve(manager: CanvasManager, port: u16, shutdown: CancellationToken) -> Result<()> {
    let server = Arc::new(ControlServer::new(manager, port)?);
    let app = Router::new()
        .route("/canvas/status", get(status))
        .route("/canvas/pair", post(pair))
        .route("/canvas/sessions", get(sessions))
        .route("/canvas/inspect", post(inspect))
        .route("/canvas/apply", post(apply))
        .route("/canvas/verify", post(verify))
        .route("/canvas/undo", post(undo))
        .route("/canvas/execute", post(execute))
        .route("/canvas/ws", get(ws))
        .with_state(server);
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.cancelled().await;
        })
        .await
        .map_err(Error::from)
}

async fn status(State(server): State<Arc<ControlServer>>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "endpoint": format!("http://127.0.0.1:{}/canvas", server.port),
            "websocket": format!("ws://127.0.0.1:{}/canvas/ws", server.port),
            "protocol_version": PROTOCOL_VERSION,
            "plugin_version": PLUGIN_VERSION,
            "canvas_mode": config::load_config().canvas_mode,
            "canvas_script": config::load_config().canvas_script,
            "sessions": server.manager.sessions().await,
        })),
    )
}

async fn pair(
    State(server): State<Arc<ControlServer>>,
    headers: HeaderMap,
    Json(payload): Json<PairRequest>,
) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    let ttl = Duration::from_secs(payload.ttl_seconds.unwrap_or(300).clamp(30, 300));
    match server.manager.create_pairing(ttl).await {
        Ok(pairing) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "code": pairing.code,
                "expires_at_ms": pairing.expires_at_ms,
                "websocket_url": format!("ws://127.0.0.1:{}/canvas/ws?code={}", server.port, pairing.code),
                "manual_action": "Open the fighorse canvas plugin in Figma and enter this pairing code.",
            })),
        ),
        Err(error) => (StatusCode::BAD_REQUEST, Json(canvas_error_json(error))),
    }
}

async fn sessions(
    State(server): State<Arc<ControlServer>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "sessions": server.manager.sessions().await,
        })),
    )
}

async fn inspect(
    State(server): State<Arc<ControlServer>>,
    headers: HeaderMap,
    Json(payload): Json<SessionRequest>,
) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    match server.manager.inspect(payload.session_id.as_deref()).await {
        Ok(value) => (StatusCode::OK, Json(json!({"ok": true, "result": value}))),
        Err(error) => (StatusCode::BAD_REQUEST, Json(canvas_error_json(error))),
    }
}

async fn apply(
    State(server): State<Arc<ControlServer>>,
    headers: HeaderMap,
    Json(payload): Json<ApplyRequest>,
) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    if let Err(error) = super::policy::ensure_cli_write(payload.yes) {
        return (StatusCode::FORBIDDEN, Json(canvas_error_json(error)));
    }
    let result = server.manager.apply_plan(payload.plan).await;
    (
        StatusCode::OK,
        Json(json!({"ok": result.error.is_none(), "result": result})),
    )
}

async fn verify(
    State(server): State<Arc<ControlServer>>,
    headers: HeaderMap,
    Json(payload): Json<SessionRequest>,
) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    match server.manager.inspect(payload.session_id.as_deref()).await {
        Ok(value) => (
            StatusCode::OK,
            Json(json!({"ok": true, "verification": value})),
        ),
        Err(error) => (StatusCode::BAD_REQUEST, Json(canvas_error_json(error))),
    }
}

async fn undo(
    State(server): State<Arc<ControlServer>>,
    headers: HeaderMap,
    Json(payload): Json<UndoRequest>,
) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    if let Err(error) = super::policy::ensure_cli_write(payload.yes) {
        return (StatusCode::FORBIDDEN, Json(canvas_error_json(error)));
    }
    let result = server
        .manager
        .undo(payload.session_id.as_deref(), &payload.transaction_id)
        .await;
    (
        StatusCode::OK,
        Json(json!({"ok": result.error.is_none(), "result": result})),
    )
}

async fn execute(
    State(server): State<Arc<ControlServer>>,
    headers: HeaderMap,
    Json(payload): Json<ExecuteRequest>,
) -> impl IntoResponse {
    if let Err(response) = server.authorize(&headers) {
        return response;
    }
    if let Err(error) = super::policy::ensure_script(payload.yes) {
        return (StatusCode::FORBIDDEN, Json(canvas_error_json(error)));
    }
    let result = server
        .manager
        .execute_script(payload.session_id.as_deref(), &payload.script)
        .await;
    (
        StatusCode::OK,
        Json(json!({"ok": result.error.is_none(), "result": result})),
    )
}

async fn ws(
    State(server): State<Arc<ControlServer>>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(server, query.code, socket))
}

async fn handle_socket(server: Arc<ControlServer>, code: String, mut socket: WebSocket) {
    let hello = match socket.recv().await {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<PluginHello>(&text) {
            Ok(hello) if hello.kind == "hello" => hello,
            _ => {
                let _ = socket
                    .send(Message::Text(
                        json!({"type": "error", "code": "invalid_hello"})
                            .to_string()
                            .into(),
                    ))
                    .await;
                return;
            }
        },
        _ => return,
    };
    let (tx, mut rx) = mpsc::channel::<CanvasPluginRequest>(32);
    let pending = Arc::new(Mutex::new(HashMap::<
        String,
        oneshot::Sender<CanvasPluginResponse>,
    >::new()));
    let now = super::now_ms();
    let session = CanvasSession {
        session_id: hello
            .session_id
            .unwrap_or_else(|| format!("session-{}", super::random_u64())),
        plugin_version: hello.plugin_version,
        editor_type: hello.editor_type,
        document_name: hello
            .document_name
            .unwrap_or_else(|| "Untitled Figma document".to_string()),
        current_page: hello.current_page,
        selection_count: hello.selection_count.unwrap_or(0),
        capabilities: hello.capabilities,
        connected_at_ms: now,
        last_heartbeat_ms: now,
    };
    let session_id = session.session_id.clone();
    match server
        .manager
        .redeem_pairing_with_channel(&code, session, tx, pending)
        .await
    {
        Ok(summary) => {
            let _ = socket
                .send(Message::Text(
                    json!({
                        "type": "paired",
                        "session": summary,
                        "protocol_version": PROTOCOL_VERSION,
                    })
                    .to_string()
                    .into(),
                ))
                .await;
        }
        Err(error) => {
            let _ = socket
                .send(Message::Text(
                    json!({"type": "error", "error": error}).to_string().into(),
                ))
                .await;
            return;
        }
    }

    loop {
        tokio::select! {
            outbound = rx.recv() => {
                let Some(outbound) = outbound else { break };
                let envelope = OutboundEnvelope {
                    kind: "request",
                    id: &outbound.id,
                    command: &outbound.command,
                    params: &outbound.params,
                };
                let payload = match serde_json::to_string(&envelope) {
                    Ok(payload) => payload,
                    Err(_) => continue,
                };
                if socket.send(Message::Text(payload.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => handle_incoming(&server.manager, &session_id, &text).await,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    server.manager.remove_session(&session_id).await;
}

async fn handle_incoming(manager: &CanvasManager, session_id: &str, text: &str) {
    if let Ok(message) = serde_json::from_str::<InboundEnvelope>(text) {
        if message.kind == "response" {
            if let Some(id) = message.id {
                let _ = manager
                    .complete_plugin_response(
                        session_id,
                        CanvasPluginResponse {
                            id,
                            result: message.result,
                            error: message.error,
                        },
                    )
                    .await;
            }
        }
    }
}

fn canvas_error_json(error: CanvasError) -> Value {
    json!({
        "ok": false,
        "error": error,
    })
}

fn random_hex() -> String {
    let bytes = [
        super::random_u64().to_ne_bytes(),
        super::random_u64().to_ne_bytes(),
    ];
    bytes
        .iter()
        .flat_map(|chunk| chunk.iter())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
