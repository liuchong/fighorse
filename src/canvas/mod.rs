//! Local Figma canvas write bridge.
//!
//! The public Figma REST API is still treated as read-oriented for node
//! structure. Native canvas writes go through an explicitly paired local Figma
//! plugin session and share the same data model between CLI and MCP.

use crate::error::Error;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, mpsc, oneshot};

pub mod control;
pub mod policy;

pub const PROTOCOL_VERSION: u32 = 1;
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

static SHARED_MANAGER: OnceLock<CanvasManager> = OnceLock::new();

pub fn shared_manager() -> CanvasManager {
    SHARED_MANAGER.get_or_init(CanvasManager::new).clone()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorType {
    #[serde(rename = "figma")]
    Figma,
    #[serde(rename = "figjam")]
    FigJam,
    #[serde(rename = "slides")]
    Slides,
}

impl EditorType {
    pub fn as_str(self) -> &'static str {
        match self {
            EditorType::Figma => "figma",
            EditorType::FigJam => "figjam",
            EditorType::Slides => "slides",
        }
    }
}

impl fmt::Display for EditorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasResultStatus {
    Applied,
    RolledBack,
    Partial,
    Unknown,
    Rejected,
}

impl CanvasResultStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CanvasResultStatus::Applied => "applied",
            CanvasResultStatus::RolledBack => "rolled_back",
            CanvasResultStatus::Partial => "partial",
            CanvasResultStatus::Unknown => "unknown",
            CanvasResultStatus::Rejected => "rejected",
        }
    }
}

impl fmt::Display for CanvasResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasErrorCode {
    ServiceUnavailable,
    PairingNotFound,
    PairingExpired,
    PairingRejected,
    SessionMissing,
    AmbiguousSession,
    EditorMismatch,
    InvalidPlan,
    PermissionDenied,
    MissingConfirmation,
    ProtocolIncompatible,
    UnsupportedOperation,
    RollbackFailed,
    TimeoutUnknown,
    AssetPathDenied,
    ScriptDisabled,
    OutputTooLarge,
    UndoConflict,
    TransportFailed,
}

impl CanvasErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            CanvasErrorCode::ServiceUnavailable => "service_unavailable",
            CanvasErrorCode::PairingNotFound => "pairing_not_found",
            CanvasErrorCode::PairingExpired => "pairing_expired",
            CanvasErrorCode::PairingRejected => "pairing_rejected",
            CanvasErrorCode::SessionMissing => "session_missing",
            CanvasErrorCode::AmbiguousSession => "ambiguous_session",
            CanvasErrorCode::EditorMismatch => "editor_mismatch",
            CanvasErrorCode::InvalidPlan => "invalid_plan",
            CanvasErrorCode::PermissionDenied => "permission_denied",
            CanvasErrorCode::MissingConfirmation => "missing_confirmation",
            CanvasErrorCode::ProtocolIncompatible => "protocol_incompatible",
            CanvasErrorCode::UnsupportedOperation => "unsupported_operation",
            CanvasErrorCode::RollbackFailed => "rollback_failed",
            CanvasErrorCode::TimeoutUnknown => "timeout_unknown",
            CanvasErrorCode::AssetPathDenied => "asset_path_denied",
            CanvasErrorCode::ScriptDisabled => "script_disabled",
            CanvasErrorCode::OutputTooLarge => "output_too_large",
            CanvasErrorCode::UndoConflict => "undo_conflict",
            CanvasErrorCode::TransportFailed => "transport_failed",
        }
    }
}

impl fmt::Display for CanvasErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasError {
    pub code: CanvasErrorCode,
    pub message: String,
}

impl CanvasError {
    pub fn new(code: CanvasErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "code": self.code,
            "message": self.message,
        })
    }
}

impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CanvasError {}

impl From<CanvasError> for Error {
    fn from(value: CanvasError) -> Self {
        Error::Usage(value.to_string())
    }
}

pub type CanvasResult<T> = std::result::Result<T, CanvasError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasOperation {
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_id: Option<String>,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CanvasVerifyOptions {
    #[serde(default)]
    pub capture: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasPlan {
    #[serde(default = "default_plan_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_editor: Option<EditorType>,
    #[serde(default)]
    pub operations: Vec<CanvasOperation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<CanvasVerifyOptions>,
}

fn default_plan_version() -> u32 {
    PROTOCOL_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasOperationResult {
    pub op_id: Option<String>,
    pub op: String,
    pub status: CanvasResultStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasApplyResult {
    pub transaction_id: String,
    pub session_id: String,
    pub status: CanvasResultStatus,
    #[serde(default)]
    pub operations: Vec<CanvasOperationResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CanvasError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasPluginResponse {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CanvasError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasSession {
    pub session_id: String,
    pub plugin_version: String,
    pub editor_type: EditorType,
    pub document_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_page: Option<String>,
    #[serde(default)]
    pub selection_count: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub connected_at_ms: u64,
    pub last_heartbeat_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasSessionSummary {
    pub session_id: String,
    pub plugin_version: String,
    pub editor_type: EditorType,
    pub document_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_page: Option<String>,
    #[serde(default)]
    pub selection_count: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub connected_at_ms: u64,
    pub last_heartbeat_ms: u64,
}

impl From<CanvasSession> for CanvasSessionSummary {
    fn from(value: CanvasSession) -> Self {
        Self {
            session_id: value.session_id,
            plugin_version: value.plugin_version,
            editor_type: value.editor_type,
            document_name: value.document_name,
            current_page: value.current_page,
            selection_count: value.selection_count,
            capabilities: value.capabilities,
            connected_at_ms: value.connected_at_ms,
            last_heartbeat_ms: value.last_heartbeat_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasPairing {
    pub code: String,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone)]
struct PairingState {
    code: String,
    expires_at_ms: u64,
}

#[derive(Debug)]
pub struct CanvasPluginRequest {
    pub id: String,
    pub command: String,
    pub params: Value,
}

#[derive(Debug, Clone)]
struct LiveSession {
    tx: mpsc::Sender<CanvasPluginRequest>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<CanvasPluginResponse>>>>,
}

#[derive(Debug, Default)]
struct ManagerState {
    pairings: HashMap<String, PairingState>,
    sessions: HashMap<String, CanvasSessionSummary>,
    live_sessions: HashMap<String, LiveSession>,
    transactions: HashMap<String, CanvasApplyResult>,
}

#[derive(Debug, Clone, Default)]
pub struct CanvasManager {
    state: Arc<Mutex<ManagerState>>,
}

impl CanvasManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_for_tests() -> Self {
        Self::new()
    }

    pub async fn create_pairing(&self, ttl: Duration) -> CanvasResult<CanvasPairing> {
        let expires_at_ms = now_ms().saturating_add(ttl.as_millis() as u64);
        let code = random_pairing_code();
        let pairing = CanvasPairing {
            code: code.clone(),
            expires_at_ms,
        };
        let mut state = self.state.lock().await;
        state.pairings.insert(
            code.clone(),
            PairingState {
                code,
                expires_at_ms,
            },
        );
        Ok(pairing)
    }

    pub async fn redeem_pairing(
        &self,
        code: &str,
        session: CanvasSession,
    ) -> CanvasResult<CanvasSessionSummary> {
        self.redeem_pairing_inner(code, session, None).await
    }

    pub async fn redeem_pairing_with_channel(
        &self,
        code: &str,
        session: CanvasSession,
        tx: mpsc::Sender<CanvasPluginRequest>,
        pending: Arc<Mutex<HashMap<String, oneshot::Sender<CanvasPluginResponse>>>>,
    ) -> CanvasResult<CanvasSessionSummary> {
        self.redeem_pairing_inner(code, session, Some(LiveSession { tx, pending }))
            .await
    }

    async fn redeem_pairing_inner(
        &self,
        code: &str,
        session: CanvasSession,
        live: Option<LiveSession>,
    ) -> CanvasResult<CanvasSessionSummary> {
        let mut state = self.state.lock().await;
        let pairing = state.pairings.remove(code).ok_or_else(|| {
            CanvasError::new(
                CanvasErrorCode::PairingNotFound,
                "Pairing code was not found or has already been used.",
            )
        })?;
        if pairing.code != code {
            return Err(CanvasError::new(
                CanvasErrorCode::PairingRejected,
                "Pairing code mismatch.",
            ));
        }
        if pairing.expires_at_ms <= now_ms() {
            return Err(CanvasError::new(
                CanvasErrorCode::PairingExpired,
                "Pairing code has expired.",
            ));
        }
        if session.plugin_version.split('.').next()
            != Some(env!("CARGO_PKG_VERSION").split('.').next().unwrap_or("0"))
        {
            return Err(CanvasError::new(
                CanvasErrorCode::ProtocolIncompatible,
                "Canvas plugin major version is incompatible; reinstall the plugin bundle.",
            ));
        }
        let summary = CanvasSessionSummary::from(session);
        state
            .sessions
            .insert(summary.session_id.clone(), summary.clone());
        if let Some(live) = live {
            state.live_sessions.insert(summary.session_id.clone(), live);
        }
        Ok(summary)
    }

    pub async fn register_test_session(&self, session: CanvasSessionSummary) {
        self.state
            .lock()
            .await
            .sessions
            .insert(session.session_id.clone(), session);
    }

    pub async fn sessions(&self) -> Vec<CanvasSessionSummary> {
        let mut sessions: Vec<_> = self.state.lock().await.sessions.values().cloned().collect();
        sessions.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        sessions
    }

    pub async fn remove_session(&self, session_id: &str) {
        let mut state = self.state.lock().await;
        state.sessions.remove(session_id);
        state.live_sessions.remove(session_id);
    }

    pub async fn resolve_session_for_plan(
        &self,
        plan: &CanvasPlan,
    ) -> CanvasResult<CanvasSessionSummary> {
        let state = self.state.lock().await;
        if let Some(session_id) = &plan.session_id {
            return state.sessions.get(session_id).cloned().ok_or_else(|| {
                CanvasError::new(
                    CanvasErrorCode::SessionMissing,
                    format!("Canvas session {session_id} is not connected."),
                )
            });
        }
        match state.sessions.len() {
            0 => Err(CanvasError::new(
                CanvasErrorCode::SessionMissing,
                "No Figma canvas plugin session is connected.",
            )),
            1 => Ok(state
                .sessions
                .values()
                .next()
                .cloned()
                .expect("one session")),
            _ => Err(CanvasError::new(
                CanvasErrorCode::AmbiguousSession,
                "Multiple Figma files are connected; pass session_id explicitly.",
            )),
        }
    }

    pub async fn remember_result(&self, result: CanvasApplyResult) {
        self.state
            .lock()
            .await
            .transactions
            .insert(result.transaction_id.clone(), result);
    }

    pub async fn transaction(&self, transaction_id: &str) -> Option<CanvasApplyResult> {
        self.state
            .lock()
            .await
            .transactions
            .get(transaction_id)
            .cloned()
    }

    pub async fn complete_plugin_response(
        &self,
        session_id: &str,
        response: CanvasPluginResponse,
    ) -> CanvasResult<()> {
        let live = {
            let state = self.state.lock().await;
            state.live_sessions.get(session_id).cloned()
        }
        .ok_or_else(|| {
            CanvasError::new(
                CanvasErrorCode::SessionMissing,
                format!("Canvas session {session_id} is not connected."),
            )
        })?;
        let sender = live.pending.lock().await.remove(&response.id);
        if let Some(sender) = sender {
            let _ = sender.send(response);
        }
        Ok(())
    }

    async fn send_plugin_command(
        &self,
        session_id: &str,
        command: &str,
        params: Value,
        timeout: Duration,
    ) -> CanvasResult<CanvasPluginResponse> {
        let live = {
            let state = self.state.lock().await;
            state.live_sessions.get(session_id).cloned()
        }
        .ok_or_else(|| {
            CanvasError::new(
                CanvasErrorCode::ServiceUnavailable,
                format!("Canvas session {session_id} is not attached to a live plugin socket."),
            )
        })?;

        let id = format!("req-{}", random_u64());
        let (tx, rx) = oneshot::channel();
        live.pending.lock().await.insert(id.clone(), tx);
        let request = CanvasPluginRequest {
            id: id.clone(),
            command: command.to_string(),
            params,
        };
        if live.tx.send(request).await.is_err() {
            live.pending.lock().await.remove(&id);
            return Err(CanvasError::new(
                CanvasErrorCode::TransportFailed,
                "Canvas plugin socket is closed.",
            ));
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(CanvasError::new(
                CanvasErrorCode::TransportFailed,
                "Canvas plugin response channel closed.",
            )),
            Err(_) => {
                live.pending.lock().await.remove(&id);
                Err(CanvasError::new(
                    CanvasErrorCode::TimeoutUnknown,
                    "Canvas plugin did not answer before timeout; result is unknown.",
                ))
            }
        }
    }

    pub async fn apply_plan(&self, mut plan: CanvasPlan) -> CanvasApplyResult {
        let transaction_id = plan
            .transaction_id
            .clone()
            .unwrap_or_else(|| format!("txn-{}", random_u64()));
        plan.transaction_id = Some(transaction_id.clone());
        let session = match self.resolve_session_for_plan(&plan).await {
            Ok(session) => session,
            Err(error) => {
                return CanvasApplyResult {
                    transaction_id,
                    session_id: plan.session_id.unwrap_or_default(),
                    status: CanvasResultStatus::Rejected,
                    operations: vec![],
                    node_ids: vec![],
                    error: Some(error),
                    data: None,
                };
            }
        };
        if let Err(error) = validate_plan(&plan, session.editor_type) {
            return CanvasApplyResult {
                transaction_id,
                session_id: session.session_id,
                status: CanvasResultStatus::Rejected,
                operations: vec![],
                node_ids: vec![],
                error: Some(error),
                data: None,
            };
        }
        if let Err(error) = prepare_plan_assets(&mut plan) {
            return CanvasApplyResult {
                transaction_id,
                session_id: session.session_id,
                status: CanvasResultStatus::Rejected,
                operations: vec![],
                node_ids: vec![],
                error: Some(error),
                data: None,
            };
        }
        let response = self
            .send_plugin_command(
                &session.session_id,
                "apply_plan",
                serde_json::to_value(&plan).unwrap_or_else(|_| json!({})),
                Duration::from_secs(120),
            )
            .await;
        let result = response_to_apply_result(transaction_id, session.session_id.clone(), response);
        self.remember_result(result.clone()).await;
        result
    }

    pub async fn inspect(&self, session_id: Option<&str>) -> CanvasResult<Value> {
        let plan = CanvasPlan {
            version: PROTOCOL_VERSION,
            transaction_id: None,
            session_id: session_id.map(str::to_string),
            expected_editor: None,
            operations: vec![],
            verify: None,
        };
        let session = self.resolve_session_for_plan(&plan).await?;
        let response = self
            .send_plugin_command(
                &session.session_id,
                "inspect",
                json!({ "session_id": session.session_id }),
                Duration::from_secs(30),
            )
            .await?;
        response
            .error
            .map(Err)
            .unwrap_or_else(|| Ok(response.result.unwrap_or_else(|| json!({}))))
    }

    pub async fn undo(&self, session_id: Option<&str>, transaction_id: &str) -> CanvasApplyResult {
        let plan = CanvasPlan {
            version: PROTOCOL_VERSION,
            transaction_id: Some(transaction_id.to_string()),
            session_id: session_id.map(str::to_string),
            expected_editor: None,
            operations: vec![],
            verify: None,
        };
        let resolved = self.resolve_session_for_plan(&plan).await;
        let session = match resolved {
            Ok(session) => session,
            Err(error) => {
                return CanvasApplyResult {
                    transaction_id: transaction_id.to_string(),
                    session_id: session_id.unwrap_or_default().to_string(),
                    status: CanvasResultStatus::Rejected,
                    operations: vec![],
                    node_ids: vec![],
                    error: Some(error),
                    data: None,
                };
            }
        };
        let response = self
            .send_plugin_command(
                &session.session_id,
                "undo",
                json!({ "transaction_id": transaction_id }),
                Duration::from_secs(30),
            )
            .await;
        response_to_apply_result(transaction_id.to_string(), session.session_id, response)
    }

    pub async fn execute_script(
        &self,
        session_id: Option<&str>,
        script: &str,
    ) -> CanvasApplyResult {
        let transaction_id = format!("txn-script-{}", random_u64());
        if script.len() > 64 * 1024 {
            return CanvasApplyResult {
                transaction_id,
                session_id: session_id.unwrap_or_default().to_string(),
                status: CanvasResultStatus::Rejected,
                operations: vec![],
                node_ids: vec![],
                error: Some(CanvasError::new(
                    CanvasErrorCode::OutputTooLarge,
                    "Canvas script must be at most 64 KiB.",
                )),
                data: None,
            };
        }
        let plan = CanvasPlan {
            version: PROTOCOL_VERSION,
            transaction_id: Some(transaction_id.clone()),
            session_id: session_id.map(str::to_string),
            expected_editor: None,
            operations: vec![],
            verify: None,
        };
        let session = match self.resolve_session_for_plan(&plan).await {
            Ok(session) => session,
            Err(error) => {
                return CanvasApplyResult {
                    transaction_id,
                    session_id: session_id.unwrap_or_default().to_string(),
                    status: CanvasResultStatus::Rejected,
                    operations: vec![],
                    node_ids: vec![],
                    error: Some(error),
                    data: None,
                };
            }
        };
        let response = self
            .send_plugin_command(
                &session.session_id,
                "execute_script",
                json!({
                    "transaction_id": transaction_id,
                    "script": script,
                }),
                Duration::from_secs(120),
            )
            .await;
        response_to_apply_result(transaction_id, session.session_id, response)
    }

    pub async fn status_json(&self) -> Value {
        json!({
            "protocol_version": PROTOCOL_VERSION,
            "plugin_version": PLUGIN_VERSION,
            "canvas_mode": crate::config::load_config().canvas_mode,
            "canvas_script": crate::config::load_config().canvas_script,
            "sessions": self.sessions().await,
        })
    }
}

fn response_to_apply_result(
    transaction_id: String,
    session_id: String,
    response: CanvasResult<CanvasPluginResponse>,
) -> CanvasApplyResult {
    match response {
        Ok(response) => {
            if let Some(error) = response.error {
                return CanvasApplyResult {
                    transaction_id,
                    session_id,
                    status: CanvasResultStatus::Rejected,
                    operations: vec![],
                    node_ids: vec![],
                    error: Some(error),
                    data: None,
                };
            }
            let result = response.result.unwrap_or_else(|| json!({}));
            match serde_json::from_value::<CanvasApplyResult>(result.clone()) {
                Ok(mut value) => {
                    if value.transaction_id.is_empty() {
                        value.transaction_id = transaction_id;
                    }
                    if value.session_id.is_empty() {
                        value.session_id = session_id;
                    }
                    value
                }
                Err(_) => CanvasApplyResult {
                    transaction_id,
                    session_id,
                    status: CanvasResultStatus::Applied,
                    operations: vec![],
                    node_ids: vec![],
                    error: None,
                    data: Some(result),
                },
            }
        }
        Err(error) if error.code == CanvasErrorCode::TimeoutUnknown => CanvasApplyResult {
            transaction_id,
            session_id,
            status: CanvasResultStatus::Unknown,
            operations: vec![],
            node_ids: vec![],
            error: Some(error),
            data: None,
        },
        Err(error) => CanvasApplyResult {
            transaction_id,
            session_id,
            status: CanvasResultStatus::Rejected,
            operations: vec![],
            node_ids: vec![],
            error: Some(error),
            data: None,
        },
    }
}

pub fn validate_plan(plan: &CanvasPlan, actual_editor: EditorType) -> CanvasResult<()> {
    if plan.version != PROTOCOL_VERSION {
        return Err(CanvasError::new(
            CanvasErrorCode::ProtocolIncompatible,
            format!(
                "Canvas plan version {} is incompatible with protocol {}.",
                plan.version, PROTOCOL_VERSION
            ),
        ));
    }
    if let Some(expected) = plan.expected_editor {
        if expected != actual_editor {
            return Err(CanvasError::new(
                CanvasErrorCode::EditorMismatch,
                format!("Plan expects {expected}, but session is {actual_editor}."),
            ));
        }
    }
    if plan.operations.len() > 200 {
        return Err(CanvasError::new(
            CanvasErrorCode::InvalidPlan,
            "Canvas plan has too many operations.",
        ));
    }
    for operation in &plan.operations {
        if operation.op.trim().is_empty() {
            return Err(CanvasError::new(
                CanvasErrorCode::InvalidPlan,
                "Canvas operation name is required.",
            ));
        }
        if !operation_allowed(actual_editor, &operation.op) {
            return Err(CanvasError::new(
                CanvasErrorCode::EditorMismatch,
                format!(
                    "Operation {} is not supported for editor {}.",
                    operation.op, actual_editor
                ),
            ));
        }
        validate_operation_args(operation)?;
    }
    Ok(())
}

pub fn prepare_plan_assets(plan: &mut CanvasPlan) -> CanvasResult<()> {
    for operation in &mut plan.operations {
        if operation.op == "place_asset" {
            prepare_asset_operation(operation)?;
        }
    }
    Ok(())
}

fn prepare_asset_operation(operation: &mut CanvasOperation) -> CanvasResult<()> {
    let path = operation
        .args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CanvasError::new(
                CanvasErrorCode::InvalidPlan,
                "place_asset requires args.path.",
            )
        })?;
    let asset = read_allowed_asset(path)?;
    let args = operation.args.as_object_mut().ok_or_else(|| {
        CanvasError::new(
            CanvasErrorCode::InvalidPlan,
            "place_asset args must be an object.",
        )
    })?;
    args.insert("path".into(), Value::String(asset.display_name));
    args.insert("mime".into(), Value::String(asset.mime));
    args.insert(
        "data_base64".into(),
        Value::String(base64_encode(&asset.bytes)),
    );
    args.insert("bytes".into(), json!(asset.bytes.len()));
    Ok(())
}

struct PreparedAsset {
    display_name: String,
    mime: String,
    bytes: Vec<u8>,
}

fn read_allowed_asset(path: &str) -> CanvasResult<PreparedAsset> {
    let original = PathBuf::from(path);
    let canonical = std::fs::canonicalize(&original).map_err(|err| {
        CanvasError::new(
            CanvasErrorCode::AssetPathDenied,
            format!("Cannot read canvas asset path: {err}"),
        )
    })?;
    if !allowed_asset_roots()
        .into_iter()
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .any(|root| canonical.starts_with(root))
    {
        return Err(CanvasError::new(
            CanvasErrorCode::AssetPathDenied,
            "Canvas assets must be under ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports.",
        ));
    }
    let metadata = std::fs::metadata(&canonical).map_err(|err| {
        CanvasError::new(
            CanvasErrorCode::AssetPathDenied,
            format!("Cannot stat canvas asset path: {err}"),
        )
    })?;
    if !metadata.is_file() || metadata.len() > 10 * 1024 * 1024 {
        return Err(CanvasError::new(
            CanvasErrorCode::AssetPathDenied,
            "Canvas asset must be a file no larger than 10 MiB.",
        ));
    }
    let mime = asset_mime(&canonical)?;
    let bytes = std::fs::read(&canonical).map_err(|err| {
        CanvasError::new(
            CanvasErrorCode::AssetPathDenied,
            format!("Cannot read canvas asset path: {err}"),
        )
    })?;
    let display_name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asset")
        .to_string();
    Ok(PreparedAsset {
        display_name,
        mime,
        bytes,
    })
}

fn allowed_asset_roots() -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        cwd.join(".fighorse").join("exports"),
        cwd.join("assets").join("fighorse"),
        home.join(".fighorse").join("exports"),
    ]
}

fn asset_mime(path: &Path) -> CanvasResult<String> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "svg" => Ok("image/svg+xml".into()),
        "png" => Ok("image/png".into()),
        "jpg" | "jpeg" => Ok("image/jpeg".into()),
        "webp" => Ok("image/webp".into()),
        "gif" => Ok("image/gif".into()),
        _ => Err(CanvasError::new(
            CanvasErrorCode::AssetPathDenied,
            "Canvas asset type must be svg, png, jpg, jpeg, webp, or gif.",
        )),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn validate_operation_args(operation: &CanvasOperation) -> CanvasResult<()> {
    if !operation.args.is_object() {
        return Err(CanvasError::new(
            CanvasErrorCode::InvalidPlan,
            format!("Operation {} args must be an object.", operation.op),
        ));
    }
    Ok(())
}

pub fn operation_allowed(editor: EditorType, op: &str) -> bool {
    common_operation(op)
        || match editor {
            EditorType::Figma => matches!(
                op,
                "create_page"
                    | "create_section"
                    | "create_frame"
                    | "set_auto_layout"
                    | "create_rectangle"
                    | "create_ellipse"
                    | "create_polygon"
                    | "create_line"
                    | "create_text"
                    | "create_component"
                    | "create_instance"
                    | "create_variant"
                    | "create_variable_collection"
                    | "bind_variable"
                    | "set_style"
            ),
            EditorType::FigJam => matches!(
                op,
                "create_section"
                    | "create_sticky"
                    | "create_shape_with_text"
                    | "create_connector"
                    | "create_table"
                    | "create_code_block"
                    | "create_text"
            ),
            EditorType::Slides => matches!(
                op,
                "create_slide_row"
                    | "create_slide"
                    | "create_text"
                    | "create_shape"
                    | "create_layout"
                    | "create_speaker_notes"
                    | "set_skip_slide"
                    | "reorder_slide"
            ),
        }
}

fn common_operation(op: &str) -> bool {
    matches!(
        op,
        "inspect"
            | "rename_node"
            | "move_node"
            | "resize_node"
            | "reparent_node"
            | "set_opacity"
            | "set_fill"
            | "set_stroke"
            | "duplicate_node"
            | "delete_node"
            | "place_asset"
            | "capture"
            | "verify"
    )
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn random_pairing_code() -> String {
    format!("pair-{:016x}{:016x}", random_u64(), random_u64())
}

pub(crate) fn random_u64() -> u64 {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut buf = [0_u8; 8];
        if std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut buf))
            .is_ok()
        {
            return u64::from_ne_bytes(buf);
        }
    }
    let time = now_ms();
    let pid = std::process::id() as u64;
    time.rotate_left(17) ^ pid.rotate_left(7)
}
