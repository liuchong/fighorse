//! MCP protocol implementation: tool registry, dispatch, resources, prompts.
//!
//! The core (`tools`, `resources`, `registry`, `policy`) is transport-agnostic
//! and returns plain JSON. The `server` module wires these onto rmcp transports
//! (stdio + Streamable HTTP) with the singleton lock and lifecycle handling.

pub mod handler;
pub mod policy;
pub mod registry;
pub mod resources;
pub mod server;
pub mod tools;
