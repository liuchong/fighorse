//! CLI argument parsing helpers.
//!
//! Mirrors the hand-rolled scheme in `fighorse.core`: positional args plus
//! `--flag value` pairs, with flag keys normalized (`--max-tokens` -> `max_tokens`).

use std::collections::HashMap;

/// Parsed flags plus the remaining positional/unconsumed args.
pub struct Flags {
    pub values: HashMap<String, String>,
    pub rest: Vec<String>,
}

impl Flags {
    /// Flag value by normalized key (e.g. `max_tokens`).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Positional argument at `idx` in the remaining args.
    pub fn arg(&self, idx: usize) -> Option<&str> {
        self.rest.get(idx).map(|s| s.as_str())
    }
}

fn normalize_flag(flag: &str) -> String {
    // Strip leading "--" then replace "-" with "_".
    flag.trim_start_matches("--").replace('-', "_")
}

/// Extract the value following `flag`, returning it and the args with both the
/// flag and its value removed. Mirrors `parse-flag`.
fn parse_flag(args: &[String], flag: &str) -> (Option<String>, Vec<String>) {
    if let Some(idx) = args.iter().position(|a| a == flag) {
        let value = args.get(idx + 1).cloned();
        let cleaned: Vec<String> = args
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != idx && *i != idx + 1)
            .map(|(_, a)| a.clone())
            .collect();
        (value, cleaned)
    } else {
        (None, args.to_vec())
    }
}

/// Parse a set of `--flag value` pairs out of `args`, mirroring `parse-flags`.
pub fn parse_flags(args: &[String], flags: &[&str]) -> Flags {
    let mut remaining = args.to_vec();
    let mut values = HashMap::new();
    for flag in flags {
        let (val, cleaned) = parse_flag(&remaining, flag);
        remaining = cleaned;
        if let Some(v) = val {
            values.insert(normalize_flag(flag), v);
        }
    }
    Flags {
        values,
        rest: remaining,
    }
}

/// True when a bare flag is present anywhere in `args`.
pub fn flag_present(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

/// Parse an integer flag value, returning `None` when absent or malformed.
pub fn optional_int(s: Option<&str>) -> Option<i64> {
    s.and_then(|v| v.trim().parse::<i64>().ok())
}

/// Parse a float flag value, returning `None` when absent or malformed.
pub fn optional_float(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| v.trim().parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_vec(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_flag_and_positional() {
        let args = to_vec(&["file123", "--depth", "2", "--output", "out.json"]);
        let flags = parse_flags(&args, &["--depth", "--output"]);
        assert_eq!(flags.get("depth"), Some("2"));
        assert_eq!(flags.get("output"), Some("out.json"));
        assert_eq!(flags.arg(0), Some("file123"));
    }

    #[test]
    fn normalizes_dashed_flag_keys() {
        let args = to_vec(&["--max-tokens", "8000"]);
        let flags = parse_flags(&args, &["--max-tokens"]);
        assert_eq!(flags.get("max_tokens"), Some("8000"));
    }

    #[test]
    fn detects_bare_flag() {
        let args = to_vec(&["file", "--manifest"]);
        assert!(flag_present(&args, "--manifest"));
        assert!(!flag_present(&args, "--apply"));
    }
}
