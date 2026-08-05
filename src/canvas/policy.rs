//! Canvas-specific permission gates.

use super::{CanvasError, CanvasErrorCode, CanvasResult};
use crate::config;

pub const YES_REQUIRED: &str =
    "Pass yes=true for MCP or --yes for CLI to confirm this canvas write.";

pub fn canvas_write_enabled() -> bool {
    config::canvas_write_enabled()
}

pub fn canvas_script_enabled() -> bool {
    config::canvas_script_enabled()
}

pub fn ensure_cli_write(yes: bool) -> CanvasResult<()> {
    if !canvas_write_enabled() {
        return Err(CanvasError::new(
            CanvasErrorCode::PermissionDenied,
            "Canvas write mode is disabled. Set FIGHORSE_CANVAS_MODE=write.",
        ));
    }
    if !yes {
        return Err(CanvasError::new(
            CanvasErrorCode::MissingConfirmation,
            YES_REQUIRED,
        ));
    }
    Ok(())
}

pub fn ensure_mcp_write(yes: bool) -> CanvasResult<()> {
    if !config::mcp_write_enabled() {
        return Err(CanvasError::new(
            CanvasErrorCode::PermissionDenied,
            "MCP Figma write mode is disabled. Set FIGHORSE_MCP_MODE=write.",
        ));
    }
    ensure_cli_write(yes)
}

pub fn ensure_script(yes: bool) -> CanvasResult<()> {
    ensure_mcp_write(yes)?;
    if !canvas_script_enabled() {
        return Err(CanvasError::new(
            CanvasErrorCode::ScriptDisabled,
            "Canvas script execution is disabled. Set FIGHORSE_CANVAS_SCRIPT=allow.",
        ));
    }
    Ok(())
}
