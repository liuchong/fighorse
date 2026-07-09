//! Generated MCP registry for official Figma REST operations.
//!
//! Generated MCP registry for official Figma REST operations.

use crate::api::coverage::{self, Operation};
use serde_json::{json, Value};

/// Build an official-operation tool definition.
pub fn official_tool(op: &Operation) -> Value {
    let deprecated_note = if op.deprecated {
        " (deprecated by Figma)."
    } else {
        ""
    };
    json!({
        "name": coverage::mcp_tool_name(op.operation_id),
        "description": format!(
            "Official Figma REST API operation {}: {} {}{}",
            op.operation_id, op.method, op.path, deprecated_note
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "params": {"type": "object", "description": "Path and query parameters, using official OpenAPI names such as file_key, ids, webhook_id, cursor."},
                "body": {"type": "object", "description": "JSON request body for POST/PUT/DELETE operations when required."},
                "ai_guidance": {"type": "boolean", "description": "When true, return a fighorse envelope with AI next-step guidance."}
            },
            "required": ["params"]
        }
    })
}

/// All official-operation tool definitions.
pub fn official_tools() -> Vec<Value> {
    coverage::official_operations()
        .iter()
        .map(official_tool)
        .collect()
}

/// Whether a tool name maps to a covered official operation.
pub fn official_tool_name(name: &str) -> bool {
    coverage::operation_for_tool_name(name).is_some()
}

/// The operationId for a `figma_*` tool name.
pub fn operation_id_for_tool(name: &str) -> Option<String> {
    coverage::operation_for_tool_name(name).map(|o| o.operation_id.to_string())
}

/// Whether a `figma_*` tool corresponds to a write operation.
pub fn write_tool_name(name: &str) -> bool {
    coverage::operation_for_tool_name(name)
        .map(|o| o.is_write())
        .unwrap_or(false)
}

/// The list of MCP resources.
pub fn resources() -> Value {
    json!([
        {"uri": "fighorse://capabilities", "name": "fighorse capabilities", "description": "Machine-readable fighorse discovery and capability manifest.", "mimeType": "application/json"},
        {"uri": "fighorse://coverage", "name": "Figma REST API coverage", "description": "Machine-readable coverage report for the official Figma REST OpenAPI snapshot.", "mimeType": "application/json"},
        {"uri": "fighorse://workflow/design-replication", "name": "Design replication workflow", "description": "Recommended fighorse workflow for AI-assisted Figma implementation.", "mimeType": "application/json"},
        {"uri": "fighorse://experience/summary", "name": "Experience summary", "description": "Prompt-ready local learned experience summary.", "mimeType": "application/json"}
    ])
}

/// The list of MCP prompts.
pub fn prompts() -> Value {
    json!([
        {"name": "fighorse_design_replication", "description": "Use fighorse to inspect a Figma URL, export assets, implement, visually verify, and record reusable lessons.",
         "arguments": [
            {"name": "figma_url", "description": "Figma design/file/proto URL", "required": true},
            {"name": "platform", "description": "Target platform/framework", "required": false},
            {"name": "asset_format", "description": "Preferred asset format", "required": false}
         ]},
        {"name": "fighorse_api_coverage", "description": "Audit fighorse against the official Figma REST OpenAPI coverage report.", "arguments": []}
    ])
}
