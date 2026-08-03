//! Canonical renderers for supported AI client MCP configurations.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientKind {
    Cursor,
    Kimi,
    Claude,
    Codex,
}

impl ClientKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "cursor" => Ok(Self::Cursor),
            "kimi" | "kimi-cli" => Ok(Self::Kimi),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            other => Err(Error::Usage(format!(
                "Unsupported install client: {other}. Expected cursor, kimi, claude, or codex."
            ))),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Kimi => "kimi",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Transport {
    Http,
    Stdio {
        command: String,
        fighorse_home: PathBuf,
    },
}

/// A client-native MCP configuration rendered from one source for both review
/// artifacts and applied user configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSpec {
    pub kind: ClientKind,
    pub url: String,
    transport: Transport,
}

impl ClientSpec {
    pub fn new(kind: ClientKind, url: impl Into<String>) -> Self {
        Self {
            kind,
            url: url.into(),
            transport: Transport::Http,
        }
    }

    pub fn explicit_stdio(
        kind: ClientKind,
        command: impl Into<String>,
        fighorse_home: impl Into<PathBuf>,
    ) -> Self {
        Self {
            kind,
            url: String::new(),
            transport: Transport::Stdio {
                command: command.into(),
                fighorse_home: fighorse_home.into(),
            },
        }
    }

    pub fn from_transport(
        kind: ClientKind,
        transport: &str,
        endpoint: impl Into<String>,
        command: impl Into<String>,
        fighorse_home: impl Into<PathBuf>,
    ) -> Result<Self> {
        match transport {
            "http" => Ok(Self::new(kind, endpoint)),
            "stdio" => Ok(Self::explicit_stdio(kind, command, fighorse_home)),
            "sse" => Err(Error::Usage(
                "Legacy SSE transport is retired. Use --transport http and the /mcp endpoint."
                    .into(),
            )),
            other => Err(Error::Usage(format!(
                "Unknown client transport: {other}. Expected http or explicit stdio."
            ))),
        }
    }

    /// Native JSON payload for Cursor, Kimi, or Claude. Codex callers should
    /// use [`Self::toml_payload`].
    pub fn json_payload(&self) -> Value {
        match &self.transport {
            Transport::Http => match self.kind {
                ClientKind::Cursor => json!({"url": self.url}),
                ClientKind::Kimi => json!({"transport": "http", "url": self.url}),
                ClientKind::Claude => json!({"type": "http", "url": self.url}),
                ClientKind::Codex => json!({"url": self.url}),
            },
            Transport::Stdio {
                command,
                fighorse_home,
            } => {
                let base = json!({
                    "command": command,
                    "args": ["mcp", "serve", "--transport", "stdio"],
                    "env": {
                        "FIGHORSE_HOME": fighorse_home,
                        "FIGHORSE_MCP_MODE": "readonly",
                        "FIGHORSE_MCP_LOCAL_WRITE": "deny",
                        "FIGHORSE_MCP_CODE_CONNECT": "deny"
                    }
                });
                match self.kind {
                    ClientKind::Claude => {
                        let mut object = base.as_object().cloned().unwrap_or_default();
                        object.insert("type".into(), json!("stdio"));
                        Value::Object(object)
                    }
                    _ => base,
                }
            }
        }
    }

    pub fn toml_payload(&self) -> String {
        match &self.transport {
            Transport::Http => format!(
                "[mcp_servers.fighorse]\nurl = \"{}\"\nenabled = true\nstartup_timeout_sec = 60\n\n[mcp_servers.fighorse.tools.discover_fighorse]\napproval_mode = \"approve\"\n",
                toml_escape(&self.url)
            ),
            Transport::Stdio {
                command,
                fighorse_home,
            } => format!(
                "[mcp_servers.fighorse]\ncommand = \"{}\"\nargs = [\"mcp\", \"serve\", \"--transport\", \"stdio\"]\nenabled = true\nstartup_timeout_sec = 60\n\n[mcp_servers.fighorse.env]\nFIGHORSE_HOME = \"{}\"\nFIGHORSE_MCP_MODE = \"readonly\"\nFIGHORSE_MCP_LOCAL_WRITE = \"deny\"\nFIGHORSE_MCP_CODE_CONNECT = \"deny\"\n\n[mcp_servers.fighorse.tools.discover_fighorse]\napproval_mode = \"approve\"\n",
                toml_escape(command),
                toml_escape(&fighorse_home.to_string_lossy())
            ),
        }
    }

    pub fn review_content(&self) -> Result<String> {
        if self.kind == ClientKind::Codex {
            Ok(self.toml_payload())
        } else {
            Ok(format!(
                "{}\n",
                serde_json::to_string_pretty(&json!({
                    "mcpServers": {"fighorse": self.json_payload()}
                }))?
            ))
        }
    }

    /// Merge the canonical payload into an existing client config while
    /// preserving all unknown user fields.
    pub fn merge_config(&self, existing: Option<&str>) -> Result<String> {
        if self.kind == ClientKind::Codex {
            return merge_codex(
                existing.unwrap_or_default(),
                &self.toml_payload(),
                (!self.url.is_empty()).then_some(self.url.as_str()),
            );
        }
        let mut root = match existing {
            Some(text) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)?
                .as_object()
                .cloned()
                .ok_or_else(|| Error::Usage("Client config must be a JSON object.".into()))?,
            _ => Map::new(),
        };
        let servers = root
            .entry("mcpServers")
            .or_insert_with(|| Value::Object(Map::new()));
        let servers = servers
            .as_object_mut()
            .ok_or_else(|| Error::Usage("mcpServers must be a JSON object.".into()))?;
        servers.insert("fighorse".into(), self.json_payload());
        Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&Value::Object(root))?
        ))
    }

    /// Verify only the fighorse-owned MCP entry so client-owned state may
    /// evolve without invalidating the installation manifest.
    pub fn matches_config(&self, existing: &str) -> bool {
        if self.kind == ClientKind::Codex {
            return self
                .merge_config(Some(existing))
                .is_ok_and(|merged| merged == existing);
        }
        serde_json::from_str::<Value>(existing)
            .ok()
            .and_then(|root| root.pointer("/mcpServers/fighorse").cloned())
            .is_some_and(|configured| configured == self.json_payload())
    }
}

pub fn config_path(home: &Path, kind: ClientKind) -> PathBuf {
    match kind {
        ClientKind::Cursor => home.join(".cursor").join("mcp.json"),
        ClientKind::Kimi => home.join(".kimi").join("mcp.json"),
        ClientKind::Claude => home.join(".claude.json"),
        ClientKind::Codex => home.join(".codex").join("config.toml"),
    }
}

fn merge_codex(existing: &str, payload: &str, desired_url: Option<&str>) -> Result<String> {
    const BEGIN: &str = "# BEGIN fighorse managed";
    const END: &str = "# END fighorse managed";
    let block = format!("{BEGIN}\n{payload}{END}\n");
    if let Some(start) = existing.find(BEGIN) {
        let tail = &existing[start..];
        let end = tail
            .find(END)
            .map(|offset| start + offset + END.len())
            .ok_or_else(|| Error::Usage("Incomplete fighorse managed TOML block.".into()))?;
        let suffix = existing[end..]
            .strip_prefix('\n')
            .unwrap_or(&existing[end..]);
        return Ok(format!("{}{}{}", &existing[..start], block, suffix));
    }
    if let Some((start, end)) = codex_fighorse_section(existing) {
        let current = &existing[start..end];
        let exact_payload = current.trim() == payload.trim();
        let current_url = toml_string_assignment(current, "url");
        let has_command = toml_string_assignment(current, "command").is_some();
        let equivalent_http = desired_url.is_some_and(|desired| {
            current_url.as_deref() == Some(desired)
                && !has_command
                && codex_tool_approval(existing, "discover_fighorse") == Some("approve")
        });
        if exact_payload || equivalent_http {
            return Ok(existing.to_string());
        }
        let preapproval_http =
            desired_url.is_some_and(|desired| known_preapproval_codex_http(current, desired));
        if known_legacy_codex_fighorse(current, current_url.as_deref()) || preapproval_http {
            return Ok(format!(
                "{}{}{}",
                &existing[..start],
                block,
                &existing[end..]
            ));
        }
        return Err(Error::Usage(
            "Existing user-managed [mcp_servers.fighorse] block was not overwritten.".into(),
        ));
    }
    if existing.trim().is_empty() {
        Ok(block)
    } else {
        Ok(format!("{}\n\n{block}", existing.trim_end()))
    }
}

fn codex_fighorse_section(existing: &str) -> Option<(usize, usize)> {
    const HEADER: &str = "[mcp_servers.fighorse]";
    let mut start = None;
    let mut offset = 0;
    for line in existing.split_inclusive('\n') {
        let trimmed = line.trim();
        match start {
            None if trimmed == HEADER => start = Some(offset),
            Some(section_start)
                if trimmed.starts_with('[')
                    && trimmed.ends_with(']')
                    && trimmed != HEADER
                    && !trimmed.starts_with("[mcp_servers.fighorse.") =>
            {
                return Some((section_start, offset));
            }
            _ => {}
        }
        offset += line.len();
    }
    start.map(|section_start| (section_start, existing.len()))
}

fn toml_string_assignment(section: &str, key: &str) -> Option<String> {
    section.lines().find_map(|line| {
        let (candidate, value) = line.trim().split_once('=')?;
        if candidate.trim() != key {
            return None;
        }
        let value = value.trim();
        if value.len() < 2 {
            return None;
        }
        let quote = value.as_bytes()[0];
        if !matches!(quote, b'"' | b'\'') || value.as_bytes()[value.len() - 1] != quote {
            return None;
        }
        Some(value[1..value.len() - 1].to_string())
    })
}

fn known_legacy_codex_fighorse(section: &str, url: Option<&str>) -> bool {
    let legacy_url = matches!(
        url,
        Some(
            "http://127.0.0.1:9449/sse"
                | "http://localhost:9449/sse"
                | "http://127.0.0.1:9449/messages"
                | "http://localhost:9449/messages"
        )
    ) && known_codex_http_fields(section);
    let legacy_stdio = toml_string_assignment(section, "command")
        .is_some_and(|command| command.to_ascii_lowercase().contains("fighorse"))
        && section.contains("args")
        && section.contains("\"mcp\"")
        && section.contains("\"serve\"")
        && known_codex_stdio_fields(section);
    legacy_url || legacy_stdio
}

fn known_preapproval_codex_http(section: &str, desired_url: &str) -> bool {
    if toml_string_assignment(section, "url").as_deref() != Some(desired_url)
        || toml_string_assignment(section, "command").is_some()
    {
        return false;
    }
    known_codex_http_fields(section)
}

fn known_codex_http_fields(section: &str) -> bool {
    section.lines().all(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed == "[mcp_servers.fighorse]" {
            return true;
        }
        trimmed
            .split_once('=')
            .map(|(key, _)| matches!(key.trim(), "url" | "enabled" | "startup_timeout_sec"))
            .unwrap_or(false)
    })
}

fn known_codex_stdio_fields(section: &str) -> bool {
    enum Table {
        Root,
        Env,
    }

    let mut table = Table::Root;
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match trimmed {
            "[mcp_servers.fighorse]" => {
                table = Table::Root;
                continue;
            }
            "[mcp_servers.fighorse.env]" => {
                table = Table::Env;
                continue;
            }
            _ if trimmed.starts_with('[') && trimmed.ends_with(']') => return false,
            _ => {}
        }
        let Some((key, _)) = trimmed.split_once('=') else {
            return false;
        };
        let known = match table {
            Table::Root => matches!(
                key.trim(),
                "command" | "args" | "enabled" | "startup_timeout_sec"
            ),
            Table::Env => matches!(
                key.trim(),
                "FIGHORSE_HOME"
                    | "FIGHORSE_MCP_MODE"
                    | "FIGHORSE_MCP_LOCAL_WRITE"
                    | "FIGHORSE_MCP_CODE_CONNECT"
            ),
        };
        if !known {
            return false;
        }
    }
    true
}

fn codex_tool_approval<'a>(config: &'a str, tool: &str) -> Option<&'a str> {
    let header = format!("[mcp_servers.fighorse.tools.{tool}]");
    let section = config.split_once(&header)?.1;
    let section = section
        .split_once("\n[")
        .map(|(current, _)| current)
        .unwrap_or(section);
    section.lines().find_map(|line| {
        let (key, value) = line.trim().split_once('=')?;
        if key.trim() != "approval_mode" {
            return None;
        }
        let value = value.trim();
        value.strip_prefix('"')?.strip_suffix('"')
    })
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
