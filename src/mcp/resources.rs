//! MCP resources and prompts for AI clients that support them.

use crate::api::coverage;
use crate::discovery;
use crate::error::{Error, Result};
use crate::experience::{self, Filters, ScopeOpts};
use crate::mcp::registry;
use serde_json::{json, Value};

/// List available resources.
pub fn list_resources() -> Value {
    json!({ "resources": registry::resources() })
}

/// Read a resource by URI, returning MCP `contents`.
pub fn read_resource(uri: &str) -> Result<Value> {
    let payload = match uri {
        "fighorse://capabilities" => Some(discovery::manifest()),
        "fighorse://coverage" => Some(coverage::coverage_report()),
        "fighorse://workflow/design-replication" => Some(discovery::workflow()),
        "fighorse://experience/summary" => Some(experience::guidance(
            &Filters::default(),
            8,
            &ScopeOpts::default(),
        )),
        _ => None,
    };
    match payload {
        Some(p) => Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&p)?,
            }]
        })),
        None => Err(Error::Other(format!("Unknown fighorse resource: {uri}"))),
    }
}

/// List available prompts.
pub fn list_prompts() -> Value {
    json!({ "prompts": registry::prompts() })
}

/// Get a prompt by name with the provided arguments.
pub fn get_prompt(name: &str, arguments: &Value) -> Result<Value> {
    let arg = |k: &str| arguments.get(k).and_then(|v| v.as_str());
    match name {
        "fighorse_design_replication" => {
            let figma_url = arg("figma_url").unwrap_or("<figma-url>");
            let platform = arg("platform").unwrap_or("ask me if missing");
            let asset_format = arg("asset_format").unwrap_or("ask me if missing");
            let text = format!(
                "Use fighorse for this Figma design: {figma_url}\nTarget platform: {platform}\nAsset format: {asset_format}\n\nRequired workflow: discover_fighorse, list_experiences, get_design_package, export assets with manifest when needed, implement, visually compare, then record reusable lessons."
            );
            Ok(json!({
                "description": "Use fighorse to implement a Figma design with a visual feedback and learning loop.",
                "messages": [{"role": "user", "content": {"type": "text", "text": text}}]
            }))
        }
        "fighorse_api_coverage" => Ok(json!({
            "description": "Audit fighorse Figma REST API parity.",
            "messages": [{"role": "user", "content": {"type": "text", "text": "Call fighorse://coverage or figma-api coverage, then verify there are no missing or drifted public REST operations."}}]
        })),
        _ => Err(Error::Other(format!("Unknown fighorse prompt: {name}"))),
    }
}
