//! fighorse — Figma data Swiss Army knife, shaped for AI consumption.
//!
//! Library crate exposing the API client, transformers, and supporting modules.
//! The `fighorse` binary (`src/main.rs`) wires these into the CLI.

pub mod api;
pub mod cli;
pub mod config;
pub mod discovery;
pub mod error;
pub mod experience;
pub mod export;
pub mod figma;
pub mod guidance;
pub mod http;
pub mod install;
pub mod mcp;
pub mod product;
pub mod transform;
pub mod url;
