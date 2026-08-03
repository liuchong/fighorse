//! Shared MCP tool safety policy.

use crate::config;
use crate::mcp::registry;
use std::collections::HashSet;

/// Tools that write local files (require FIGHORSE_MCP_LOCAL_WRITE=allow).
pub fn local_write_tool_names() -> HashSet<&'static str> {
    ["export_images", "export_component", "download_image_fills"]
        .into_iter()
        .collect()
}

/// Whether a tool is a Figma write operation.
pub fn write_tool(legacy_write_names: &HashSet<String>, name: &str) -> bool {
    legacy_write_names.contains(name) || registry::write_tool_name(name)
}

/// Whether a tool writes local files.
pub fn local_write_tool(name: &str) -> bool {
    local_write_tool_names().contains(name)
}

pub fn code_connect_egress_tool(name: &str) -> bool {
    matches!(
        name,
        "preview_code_connect" | "publish_code_connect" | "unpublish_code_connect"
    )
}

/// Return a policy violation message for `name`, or None if allowed.
pub fn violation(legacy_write_names: &HashSet<String>, name: &str) -> Option<String> {
    if code_connect_egress_tool(name) && !config::mcp_code_connect_enabled() {
        return Some(format!(
            "Tool {name} can send Code Connect template code to Figma and requires FIGHORSE_MCP_CODE_CONNECT=allow."
        ));
    }
    if write_tool(legacy_write_names, name) && !config::mcp_write_enabled() {
        return Some(format!(
            "Tool {name} is disabled in readonly mode. Set FIGHORSE_MCP_MODE=write to enable Figma write tools."
        ));
    }
    if local_write_tool(name) && !config::mcp_local_write_enabled() {
        return Some(format!(
            "Tool {name} writes local files and requires FIGHORSE_MCP_LOCAL_WRITE=allow. Allowed output roots are ./.fighorse/exports, ./assets/fighorse, and ~/.fighorse/exports."
        ));
    }
    None
}
