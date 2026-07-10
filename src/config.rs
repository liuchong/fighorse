//! Configuration loading and defaults.
//!
//! Configuration loading: reads `~/.fighorse/config.json` (with a legacy
//! `~/.config/fighorse/config.json` fallback) and layers environment variables
//! on top.

use serde_json::Value;
use std::path::PathBuf;

/// Resolve the fighorse home directory (`$FIGHORSE_HOME` or `~/.fighorse`).
pub fn fighorse_home() -> PathBuf {
    if let Ok(home) = std::env::var("FIGHORSE_HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    home_dir().join(".fighorse")
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Path to the primary config file (`<home>/config.json`).
pub fn config_path() -> PathBuf {
    fighorse_home().join("config.json")
}

/// Path to the legacy config file (`~/.config/fighorse/config.json`).
pub fn legacy_config_path() -> PathBuf {
    home_dir()
        .join(".config")
        .join("fighorse")
        .join("config.json")
}

fn read_json_file(path: &PathBuf) -> serde_json::Map<String, Value> {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        },
        Err(_) => serde_json::Map::new(),
    }
}

fn load_file_config() -> serde_json::Map<String, Value> {
    let mut merged = read_json_file(&legacy_config_path());
    for (k, v) in read_json_file(&config_path()) {
        merged.insert(k, v);
    }
    merged
}

fn str_field(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

/// Resolved configuration for the current process.
#[derive(Debug, Clone)]
pub struct Config {
    pub token: Option<String>,
    pub config_path: PathBuf,
    pub legacy_config_path: PathBuf,
    pub fighorse_home: PathBuf,
    pub proxy: Option<String>,
    pub mcp_mode: String,
    pub mcp_local_write: String,
}

/// Save user config under `<home>/config.json`.
pub fn save_config(config: &Value) -> crate::error::Result<()> {
    let file = config_path();
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&file, content)?;
    Ok(())
}

/// Remove the saved config file if present.
pub fn clear_config() -> crate::error::Result<()> {
    let file = config_path();
    if file.exists() {
        std::fs::remove_file(&file)?;
    }
    Ok(())
}

/// Load merged configuration from files + environment variables.
pub fn load_config() -> Config {
    let file_config = load_file_config();

    let token = env_nonempty("FIGMA_TOKEN")
        .or_else(|| env_nonempty("FIGMA_API_KEY"))
        .or_else(|| str_field(&file_config, "token"));

    let proxy = env_nonempty("HTTPS_PROXY")
        .or_else(|| env_nonempty("https_proxy"))
        .or_else(|| env_nonempty("HTTP_PROXY"))
        .or_else(|| env_nonempty("http_proxy"))
        .or_else(|| env_nonempty("ALL_PROXY"))
        .or_else(|| env_nonempty("all_proxy"));

    let mcp_mode = env_nonempty("FIGHORSE_MCP_MODE")
        .or_else(|| str_field(&file_config, "mcp-mode"))
        .unwrap_or_else(|| "readonly".to_string());

    let mcp_local_write = env_nonempty("FIGHORSE_MCP_LOCAL_WRITE")
        .or_else(|| str_field(&file_config, "mcp-local-write"))
        .unwrap_or_else(|| "deny".to_string());

    Config {
        token,
        config_path: config_path(),
        legacy_config_path: legacy_config_path(),
        fighorse_home: fighorse_home(),
        proxy,
        mcp_mode,
        mcp_local_write,
    }
}

/// True when the MCP server is allowed to perform Figma write operations.
pub fn mcp_write_enabled() -> bool {
    matches!(load_config().mcp_mode.as_str(), "write" | "full" | "unsafe")
}

/// True when the MCP server is allowed to write local files (asset exports).
pub fn mcp_local_write_enabled() -> bool {
    matches!(
        load_config().mcp_local_write.as_str(),
        "allow" | "true" | "1" | "yes"
    )
}
