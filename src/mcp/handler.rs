//! Official rmcp protocol adapter for fighorse's JSON-oriented business layer.

use crate::mcp::{resources, tools};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, GetPromptRequestParams, GetPromptResult,
        Implementation, ListPromptsResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult, ServerCapabilities,
        ServerInfo,
    },
    service::RequestContext,
    ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

const INSTRUCTIONS: &str = "Call discover_fighorse first. For Figma replication, ask when \
platform or asset_format is missing, export assets with manifests, and record reusable lessons \
after visual fixes.";

#[derive(Debug, Clone, Default)]
pub struct FighorseHandler;

fn decode_result<T: DeserializeOwned>(value: Value) -> Result<T, McpError> {
    serde_json::from_value(value).map_err(|error| McpError::internal_error(error.to_string(), None))
}

impl ServerHandler for FighorseHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("fighorse", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        decode_result(tools::list_tools())
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let arguments = request
            .arguments
            .map(Value::Object)
            .unwrap_or_else(|| json!({}));
        decode_result(tools::call_tool(request.name.as_ref(), &arguments).await)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        decode_result(resources::list_resources())
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let value = resources::read_resource(&request.uri)
            .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        decode_result(value)
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        decode_result(resources::list_prompts())
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let arguments = request
            .arguments
            .map(Value::Object)
            .unwrap_or_else(|| json!({}));
        let value = resources::get_prompt(&request.name, &arguments)
            .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        decode_result(value)
    }
}
