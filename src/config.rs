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
    pub auth_scheme: String,
    pub config_path: PathBuf,
    pub legacy_config_path: PathBuf,
    pub fighorse_home: PathBuf,
    pub proxy: Option<String>,
    pub mcp_mode: String,
    pub mcp_local_write: String,
    pub mcp_code_connect: String,
    pub canvas_mode: String,
    pub canvas_script: String,
    pub canvas_port: u16,
    pub canvas_bridge: String,
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
        .or_else(|| env_nonempty("FIGMA_ACCESS_TOKEN"))
        .or_else(|| env_nonempty("FIGMA_OAUTH_TOKEN"))
        .or_else(|| env_nonempty("FIGMA_API_KEY"))
        .or_else(|| str_field(&file_config, "token"));

    let auth_scheme = env_nonempty("FIGHORSE_AUTH_SCHEME")
        .or_else(|| str_field(&file_config, "auth-scheme"))
        .or_else(|| {
            if env_nonempty("FIGMA_OAUTH_TOKEN").is_some()
                && env_nonempty("FIGMA_TOKEN").is_none()
                && env_nonempty("FIGMA_ACCESS_TOKEN").is_none()
            {
                Some("bearer".to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "auto".to_string());

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

    let mcp_code_connect = env_nonempty("FIGHORSE_MCP_CODE_CONNECT")
        .or_else(|| str_field(&file_config, "mcp-code-connect"))
        .unwrap_or_else(|| "deny".to_string());

    let canvas_mode = env_nonempty("FIGHORSE_CANVAS_MODE")
        .or_else(|| str_field(&file_config, "canvas-mode"))
        .unwrap_or_else(|| "readonly".to_string());

    let canvas_script = env_nonempty("FIGHORSE_CANVAS_SCRIPT")
        .or_else(|| str_field(&file_config, "canvas-script"))
        .unwrap_or_else(|| "deny".to_string());

    let canvas_port = env_nonempty("FIGHORSE_CANVAS_PORT")
        .or_else(|| str_field(&file_config, "canvas-port"))
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(9450);

    let canvas_bridge = env_nonempty("FIGHORSE_CANVAS_BRIDGE")
        .or_else(|| str_field(&file_config, "canvas-bridge"))
        .unwrap_or_else(|| "deny".to_string());

    Config {
        token,
        auth_scheme,
        config_path: config_path(),
        legacy_config_path: legacy_config_path(),
        fighorse_home: fighorse_home(),
        proxy,
        mcp_mode,
        mcp_local_write,
        mcp_code_connect,
        canvas_mode,
        canvas_script,
        canvas_port,
        canvas_bridge,
    }
}

/// Resolve the HTTP authentication header name and value for a Figma token.
///
/// Figma supports three credential classes across its REST API:
/// - Personal access tokens (`figd_...`) -> `X-Figma-Token: <token>`
/// - Organization/Enterprise plan access tokens -> `X-Figma-Token: <token>`
/// - OAuth 2.0 access tokens (`figu_...` or explicit `bearer`/`oauth` scheme) -> `Authorization: Bearer <token>`
pub fn resolve_auth_header_with_scheme(token: &str, scheme: &str) -> (&'static str, String) {
    let trimmed = token.trim();
    if trimmed.len() > 7 && trimmed[..7].eq_ignore_ascii_case("bearer ") {
        return (
            "Authorization",
            format!("Bearer {}", trimmed[7..].trim()),
        );
    }
    match scheme.trim().to_ascii_lowercase().as_str() {
        "bearer" | "oauth" | "oauth2" => ("Authorization", format!("Bearer {trimmed}")),
        "pat" | "token" | "plan" | "plan_token" | "x-figma-token" => {
            ("X-Figma-Token", trimmed.to_string())
        }
        _ => {
            if trimmed.starts_with("figu_") {
                ("Authorization", format!("Bearer {trimmed}"))
            } else {
                ("X-Figma-Token", trimmed.to_string())
            }
        }
    }
}

/// Resolve the HTTP authentication header for `token` using the active `FIGHORSE_AUTH_SCHEME`.
pub fn resolve_auth_header(token: &str) -> (&'static str, String) {
    let scheme = load_config().auth_scheme;
    resolve_auth_header_with_scheme(token, &scheme)
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

/// True when the MCP server may send Code Connect template code to Figma.
pub fn mcp_code_connect_enabled() -> bool {
    matches!(
        load_config().mcp_code_connect.as_str(),
        "allow" | "true" | "1" | "yes"
    )
}

/// True when local canvas writes through the Figma plugin bridge are allowed.
pub fn canvas_write_enabled() -> bool {
    matches!(
        load_config().canvas_mode.as_str(),
        "write" | "full" | "unsafe"
    )
}

/// True when arbitrary Plugin API JavaScript may be sent to a paired session.
pub fn canvas_script_enabled() -> bool {
    matches!(
        load_config().canvas_script.as_str(),
        "allow" | "true" | "1" | "yes"
    )
}

/// True when `mcp serve` should also start the canvas control bridge.
pub fn canvas_bridge_enabled() -> bool {
    matches!(
        load_config().canvas_bridge.as_str(),
        "allow" | "true" | "1" | "yes" | "write"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_auth_header_distinguishes_oauth_pat_and_explicit_schemes() {
        assert_eq!(
            resolve_auth_header_with_scheme("figd_abc", "auto"),
            ("X-Figma-Token", "figd_abc".to_string())
        );
        assert_eq!(
            resolve_auth_header_with_scheme("plan_secret_123", "auto"),
            ("X-Figma-Token", "plan_secret_123".to_string())
        );
        assert_eq!(
            resolve_auth_header_with_scheme("figu_oauth_xyz", "auto"),
            ("Authorization", "Bearer figu_oauth_xyz".to_string())
        );
        assert_eq!(
            resolve_auth_header_with_scheme("Bearer raw_oauth", "pat"),
            ("Authorization", "Bearer raw_oauth".to_string())
        );
        assert_eq!(
            resolve_auth_header_with_scheme("custom_oauth", "bearer"),
            ("Authorization", "Bearer custom_oauth".to_string())
        );
        assert_eq!(
            resolve_auth_header_with_scheme("figu_override_as_pat", "pat"),
            ("X-Figma-Token", "figu_override_as_pat".to_string())
        );
    }
}
