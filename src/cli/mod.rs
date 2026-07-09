//! CLI runtime: output helpers, token resolution, and command dispatch.

pub mod args;
pub mod commands;

use crate::config;
use serde_json::Value;
use std::io::Read;

/// Print an error line to stderr (space-joined), matching `eprintln`.
pub fn eprintln_parts(parts: &[&str]) {
    eprintln!("{}", parts.join(" "));
}

/// Serialize data as 2-space pretty JSON, matching `js/JSON.stringify … 2`.
pub fn json_str(data: &Value) -> String {
    serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".to_string())
}

/// Write `content` to `output` file (creating parents) or stdout.
pub fn write_output(content: &str, output: Option<&str>) -> crate::error::Result<()> {
    match output {
        Some(path) => {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(path, content)?;
        }
        None => println!("{content}"),
    }
    Ok(())
}

/// Print a data value as pretty JSON to `output` or stdout.
pub fn print_data(data: &Value, output: Option<&str>) -> crate::error::Result<()> {
    write_output(&json_str(data), output)
}

/// Read all of stdin as a UTF-8 string.
pub fn read_stdin() -> String {
    let mut buf = String::new();
    let _ = std::io::stdin().read_to_string(&mut buf);
    buf
}

/// Resolve the Figma token or exit with an error, matching `require-token!`.
pub fn require_token() -> String {
    let cfg = config::load_config();
    match cfg.token {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            eprintln!("Error: FIGMA_TOKEN or FIGMA_API_KEY environment variable required");
            std::process::exit(1);
        }
    }
}

/// Require a positional arg, exiting with an error if missing/blank.
pub fn require_arg<'a>(args: &'a [String], idx: usize, name: &str) -> &'a str {
    match args.get(idx) {
        Some(v) if !v.trim().is_empty() => v.as_str(),
        _ => {
            eprintln!("Error: {name} required");
            std::process::exit(1);
        }
    }
}

/// Require a value (from flag or positional), exiting if blank.
pub fn require_value(value: Option<&str>, name: &str) -> String {
    match value {
        Some(v) if !v.trim().is_empty() => v.to_string(),
        _ => {
            eprintln!("Error: {name} required");
            std::process::exit(1);
        }
    }
}

/// Parse a JSON object from a string, exiting on malformed input. Returns an
/// empty map for blank input (matching `parse-json-map!` returning nil -> {}).
pub fn parse_json_map(raw: Option<&str>, label: &str) -> Value {
    match raw {
        None => Value::Object(Default::default()),
        Some(s) if s.trim().is_empty() => Value::Object(Default::default()),
        Some(s) => match serde_json::from_str::<Value>(s) {
            Ok(v @ Value::Object(_)) => v,
            Ok(_) => {
                eprintln!("Error: {label} must be a JSON object");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },
    }
}

/// Format an error into its display string.
pub fn err_message(e: &crate::error::Error) -> String {
    e.to_string()
}
