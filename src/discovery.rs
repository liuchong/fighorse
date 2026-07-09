//! Self-description payloads for AI tools and MCP clients.
//!
//! Runtime detection reports the running fighorse binary rather than the
//! native Rust binary: the `runtime` check reports the running fighorse binary.

use crate::api::coverage as api_coverage;
use crate::config;
use crate::experience;
use crate::guidance;
use crate::url as figma_url;
use serde_json::{json, Value};
use std::path::PathBuf;

pub const VERSION: &str = "0.1.0";

fn path_dirs() -> Vec<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    std::env::var("PATH")
        .unwrap_or_default()
        .split(sep)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn executable_candidates(command: &str) -> Vec<String> {
    let exts: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    if command.trim().is_empty() {
        return vec![];
    }
    if PathBuf::from(command).is_absolute() || command.contains('/') || command.contains('\\') {
        return exts.iter().map(|e| format!("{command}{e}")).collect();
    }
    let mut out = Vec::new();
    for dir in path_dirs() {
        for ext in exts {
            out.push(
                PathBuf::from(&dir)
                    .join(format!("{command}{ext}"))
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    out
}

fn executable_path(command: &str) -> Option<String> {
    executable_candidates(command)
        .into_iter()
        .find(|c| PathBuf::from(c).exists())
}

fn home_exists() -> bool {
    config::fighorse_home().exists()
}

fn mcp_lock_file() -> PathBuf {
    config::fighorse_home().join("runtime").join("mcp.lock")
}

fn read_json_object(file: &PathBuf) -> Option<Value> {
    let content = std::fs::read_to_string(file).ok()?;
    serde_json::from_str(&content).ok()
}

fn active_pid(pid: Option<i64>) -> bool {
    match pid {
        Some(p) if p > 0 => {
            #[cfg(unix)]
            unsafe {
                extern "C" {
                    fn kill(pid: i32, sig: i32) -> i32;
                }
                // kill(pid, 0): 0 => alive; -1 with ESRCH => dead.
                kill(p as i32, 0) == 0 || {
                    // errno != ESRCH means it exists but we can't signal it.
                    std::io::Error::last_os_error().raw_os_error() != Some(3)
                }
            }
            #[cfg(not(unix))]
            {
                false
            }
        }
        _ => false,
    }
}

/// Status of the local MCP singleton service.
pub fn mcp_service_status() -> Value {
    let lock_file = mcp_lock_file();
    let lock = read_json_object(&lock_file);
    let pid = lock
        .as_ref()
        .and_then(|l| l.get("pid"))
        .and_then(|v| v.as_i64());
    let running = active_pid(pid);
    json!({
        "endpoint": "http://127.0.0.1:9449/mcp",
        "health": "http://127.0.0.1:9449/health",
        "lock_file": lock_file.to_string_lossy(),
        "lock_present": lock.is_some(),
        "pid": pid,
        "running": running,
        "next_step": if running {
            "Ask the client to call discover_fighorse, or run fighorse doctor --format json."
        } else {
            "For MCP clients, run fighorse install --default --mode service --clients cursor,codex,kimi --apply."
        }
    })
}

/// Structured setup instructions for humans and AI clients.
pub fn setup_guidance() -> Value {
    json!({
        "kind": "fighorse.setup-guidance.v1",
        "required_for_figma_api": [{
            "id": "figma_token",
            "name": "Figma Personal Access Token",
            "why": "fighorse uses the public Figma REST API; file, node, image, component, variable, comment, webhook, analytics, and design-package calls require a token.",
            "accepted_sources": [
                "FIGMA_TOKEN environment variable",
                "FIGMA_API_KEY environment variable",
                "~/.fighorse/config.json written by fighorse auth login"
            ],
            "recommended_command": "fighorse auth login --token <FIGMA_TOKEN>",
            "one_shot_command": "FIGMA_TOKEN=<FIGMA_TOKEN> fighorse quickstart \"<figma-frame-url>\"",
            "safety": "Do not paste tokens into prompts, commits, screenshots, or shared logs. Store them through local config or environment variables only."
        }],
        "recommended_first_run": [
            "Run fighorse auth login --token <FIGMA_TOKEN>.",
            "Copy a link to a selected Figma frame, component, or group so the URL includes node-id.",
            "Run fighorse quickstart \"<figma-frame-url>\".",
            "Run fighorse design package \"<figma-frame-url>\" --platform <target-platform> --asset-format <asset-format>."
        ],
        "optional_mcp_service": {
            "when": "Only when an AI client such as Cursor, Codex, or Kimi should call fighorse directly.",
            "command": "fighorse install --default --mode service --clients cursor,codex,kimi --apply",
            "endpoint": "http://127.0.0.1:9449/mcp"
        },
        "ai_client_behavior": {
            "must_check_first": ["discover_fighorse", "check_fighorse_ready"],
            "if_auth_missing": "Do not call Figma API tools yet. Tell the user: fighorse needs a Figma Personal Access Token. Run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then retry.",
            "if_url_missing": "Ask the user to paste a specific Figma frame/group/component link, not a full canvas link.",
            "if_platform_or_asset_format_missing": "Ask the user for target platform/framework and preferred asset format before implementation.",
            "after_user_fixes_setup": "Call check_fighorse_ready again, then parse_figma_url and get_design_package."
        }
    })
}

/// The runtime name/version reported by checks. In the Rust build this is the
/// fighorse binary itself.
fn runtime_info() -> (String, Option<String>) {
    ("fighorse".to_string(), Some(VERSION.to_string()))
}

/// Read-only guided readiness report for new users.
pub fn quickstart(figma_url: Option<&str>) -> Value {
    let cfg = config::load_config();
    let has_token = cfg.token.as_deref().map(|t| !t.is_empty()).unwrap_or(false);
    let parsed = figma_url
        .filter(|u| !u.trim().is_empty())
        .map(figma_url::parse_figma_url);
    let has_url = parsed.is_some();
    let exact_selection = parsed
        .as_ref()
        .map(|p| p.node_id.is_some())
        .unwrap_or(false);
    let parsed_valid = parsed.as_ref().map(|p| p.valid).unwrap_or(false);
    let parsed_file_key = parsed.as_ref().and_then(|p| p.file_key.clone());

    let binary = executable_path("fighorse").or_else(|| {
        // Fall back to a locally built release binary during development.
        let p = PathBuf::from("./target/release/fighorse");
        if p.exists() {
            std::fs::canonicalize(&p)
                .ok()
                .map(|c| c.to_string_lossy().into_owned())
        } else {
            None
        }
    });

    let (_rt_name, rt_version) = runtime_info();
    let runtime_ok = rt_version.is_some();

    let mut checks = vec![
        json!({
            "id": "runtime",
            "ok": runtime_ok,
            "message": if runtime_ok { "fighorse runtime is available." } else { "fighorse runtime is not available." }
        }),
        json!({
            "id": "binary",
            "ok": binary.is_some(),
            "message": match &binary {
                Some(b) => format!("fighorse binary found at {b}"),
                None => "Build and install fighorse before using it globally.".to_string(),
            },
            "next_command": "From source: cargo build --release. From a downloaded binary: ./fighorse install --default --apply."
        }),
        json!({
            "id": "auth",
            "ok": has_token,
            "message": if has_token { "Figma token is configured." } else { "Figma token is missing." },
            "next_command": "fighorse auth login --token <FIGMA_TOKEN>"
        }),
    ];

    let figma_url_message = match &parsed {
        None => "Paste a Figma frame, component, or group link to continue.".to_string(),
        Some(p) if p.valid => "Figma URL or file key parsed successfully.".to_string(),
        Some(p) => p.error.clone().unwrap_or_default(),
    };
    checks.push(json!({
        "id": "figma_url",
        "ok": has_url && parsed_valid,
        "message": figma_url_message,
        "next_command": "fighorse quickstart \"<figma-frame-url>\""
    }));

    let specific_message = if exact_selection {
        "The link includes node-id, so it targets a specific selection."
    } else if has_url {
        "The link has no node-id. Copy a link to a selected frame, component, or group for best results."
    } else {
        "Copy a link to a selected frame, component, or group."
    };
    checks.push(json!({
        "id": "specific_frame",
        "ok": exact_selection,
        "message": specific_message,
        "next_command": "In Figma: right click the frame or group, then copy link to selection."
    }));

    let ready = checks.iter().all(|c| c["ok"].as_bool().unwrap_or(false));

    let design_command = if parsed_valid && parsed_file_key.is_some() {
        let target = figma_url
            .map(String::from)
            .unwrap_or_else(|| parsed_file_key.clone().unwrap());
        Some(format!(
            "fighorse design package \"{target}\" --platform <target-platform> --asset-format <asset-format> --output ./.fighorse/exports/package.json"
        ))
    } else {
        None
    };

    let mut next_steps: Vec<String> = Vec::new();
    if binary.is_none() {
        next_steps.push("Build and install from source: cargo build --release. Or install a downloaded binary: ./fighorse install --default --apply.".to_string());
    }
    if !has_token {
        next_steps.push("Add a Figma token: fighorse auth login --token <FIGMA_TOKEN>".to_string());
    }
    if !has_url {
        next_steps.push("Copy a link to a specific Figma frame, component, or group.".to_string());
    }
    if has_url && !exact_selection {
        next_steps.push("Narrow the input to an exact Figma selection with node-id.".to_string());
    }
    if let Some(dc) = &design_command {
        next_steps.push(dc.clone());
    }
    next_steps.push("Optional MCP service: fighorse install --default --mode service --clients cursor,codex,kimi --apply".to_string());

    json!({
        "kind": "fighorse.quickstart.v1",
        "status": if ready { "ready" } else { "needs-action" },
        "summary": if ready { "Ready to build a design package." } else { "Follow next_steps before building a design package." },
        "checks": checks,
        "auth": {"has_token": has_token, "config_path": cfg.config_path.to_string_lossy()},
        "install": {
            "home": config::fighorse_home().to_string_lossy(),
            "home_exists": home_exists(),
            "binary": binary,
            "default_mode": "cli",
            "service_mode": "explicit: fighorse install --default --mode service --clients cursor,codex,kimi --apply"
        },
        "mcp": mcp_service_status(),
        "setup": setup_guidance(),
        "figma_url": parsed.as_ref().map(|p| p.to_json()),
        "proxy": {"configured": cfg.proxy.is_some(), "value": cfg.proxy},
        "next_steps": next_steps,
    })
}

/// Markdown rendering of the quickstart report.
pub fn quickstart_markdown(report: &Value) -> String {
    let summary = report.get("summary").and_then(|v| v.as_str()).unwrap_or("");
    let checks: Vec<String> = report
        .get("checks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| {
                    let ok = c["ok"].as_bool().unwrap_or(false);
                    let id = c["id"].as_str().unwrap_or("");
                    let message = c["message"].as_str().unwrap_or("");
                    let mut line = format!("- {} `{id}`: {message}", if ok { "OK" } else { "TODO" });
                    if !ok {
                        if let Some(nc) = c.get("next_command").and_then(|v| v.as_str()) {
                            line.push_str(&format!("\n  Next: `{nc}`"));
                        }
                    }
                    line
                })
                .collect()
        })
        .unwrap_or_default();
    let next_steps: Vec<String> = report
        .get("next_steps")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|s| format!("- {}", s.as_str().unwrap_or("")))
                .collect()
        })
        .unwrap_or_default();

    format!(
        "# fighorse Quickstart\n\n{summary}\n\n\
## Required Figma Setup\n\n\
1. Save a Figma Personal Access Token before calling Figma APIs:\n   `fighorse auth login --token <FIGMA_TOKEN>`\n   Or run one command with `FIGMA_TOKEN=<token> fighorse ...`.\n\
2. Copy a specific Figma frame, group, or component link. Avoid whole-canvas links for implementation.\n\
3. Run quickstart again with the selected link:\n   `fighorse quickstart \"<figma-frame-url>\"`\n\
4. Build an AI-ready design package:\n   `fighorse design package \"<figma-frame-url>\" --platform <target> --asset-format <format>`\n\
5. Optional MCP service for Cursor/Codex/Kimi:\n   `fighorse install --default --mode service --clients cursor,codex,kimi --apply`\n\n\
## AI Client Setup Rule\n\n\
First run `fighorse quickstart --format json` or MCP `check_fighorse_ready`. If `auth.has_token=false`, do not call Figma API tools yet. Tell the user: fighorse needs a Figma Personal Access Token; run `fighorse auth login --token <FIGMA_TOKEN>` or set `FIGMA_TOKEN`, then retry.\n\n\
## Checks\n\n{}\n\n## Next Steps\n\n{}\n",
        checks.join("\n"),
        next_steps.join("\n")
    )
}

/// Diagnostics report.
pub fn doctor() -> Value {
    let cfg = config::load_config();
    let has_token = cfg.token.as_deref().map(|t| !t.is_empty()).unwrap_or(false);
    let (rt_name, rt_version) = runtime_info();
    let local_write = config::mcp_local_write_enabled();
    let mcp_service = mcp_service_status();
    let stale_lock = mcp_service["lock_present"].as_bool().unwrap_or(false)
        && !mcp_service["running"].as_bool().unwrap_or(false);
    let env_token_present = std::env::var("FIGMA_TOKEN").map(|v| !v.is_empty()).unwrap_or(false)
        || std::env::var("FIGMA_API_KEY").map(|v| !v.is_empty()).unwrap_or(false);
    let opts = experience::ScopeOpts::default();

    json!({
        "kind": "fighorse.doctor.v1",
        "runtime": {"name": rt_name, "version": rt_version, "ok": rt_version.is_some()},
        "mcp": {
            "mode": cfg.mcp_mode,
            "write_enabled": config::mcp_write_enabled(),
            "local_write_enabled": config::mcp_local_write_enabled(),
            "local_write_env": "FIGHORSE_MCP_LOCAL_WRITE=allow"
        },
        "install": {
            "home": cfg.fighorse_home.to_string_lossy(),
            "output_locations": guidance::output_location_guidance(),
            "next_step": "Run fighorse install status first. Install commands generate artifacts by default; pass --apply only when you want fighorse to mutate detected CLI, MCP service, AI client, and skill locations."
        },
        "experience": {
            "store_path": experience::experience_path(&opts).to_string_lossy(),
            "store": experience::store_info(&opts),
            "schema_version": experience::SCHEMA_VERSION,
            "records": experience::read_all(&opts).len(),
            "next_step": "Call fighorse experience summary or MCP list_experiences before implementing a Figma design."
        },
        "auth": {
            "has_token": has_token,
            "config_path": cfg.config_path.to_string_lossy(),
            "env_token_present": env_token_present,
            "required_for_figma_api": true,
            "setup_command": "fighorse auth login --token <FIGMA_TOKEN>",
            "missing_token_ai_prompt": "fighorse needs a Figma Personal Access Token before it can read Figma files. Please run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then ask me to retry."
        },
        "checks": [
            {"id": "token", "ok": has_token,
             "message": if has_token { "Figma token is configured." } else { "Figma token is missing; Figma API calls will fail." },
             "next_step": "Run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN for one command."},
            {"id": "mcp_service", "ok": mcp_service["running"].as_bool().unwrap_or(false),
             "message": if mcp_service["running"].as_bool().unwrap_or(false) { "Local MCP service appears to have an active singleton owner." } else { "Local MCP service is not running. This is fine for CLI-only mode." },
             "next_step": "For AI clients, run fighorse install --default --mode service --clients cursor,codex,kimi --apply."},
            {"id": "mcp_repeated_handshake", "ok": true,
             "message": "The /mcp endpoint is expected to create a fresh stateless transport/server per request, so Codex-style repeated initialize handshakes stay valid.",
             "next_step": "If a client reports text/plain during initialize, restart the installed fighorse service and verify /mcp implementation notes in AGENTS.md."},
            {"id": "local_write", "ok": local_write,
             "message": if local_write { "MCP local file export is enabled." } else { "MCP local file export is disabled by default." },
             "next_step": "Set FIGHORSE_MCP_LOCAL_WRITE=allow only when the client may write under ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports."},
            {"id": "stale_singleton_lock", "ok": !stale_lock,
             "message": if stale_lock { "A stale MCP singleton lock was found." } else { "No stale MCP singleton lock detected." },
             "next_step": format!("Remove {} only after confirming no fighorse MCP service is running.", mcp_service["lock_file"].as_str().unwrap_or(""))}
        ],
        "mcp_service": mcp_service,
        "setup": setup_guidance(),
        "troubleshooting": {
            "broad_canvas_target": "If diagnostics mention CANVAS, page, or user-flow target, copy a link to a specific frame, component, or group.",
            "token_missing": "Run fighorse auth login --token <FIGMA_TOKEN>. AI clients should surface this exact command when auth.has_token is false.",
            "export_path_rejected": "Use ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports. MCP also requires FIGHORSE_MCP_LOCAL_WRITE=allow.",
            "mcp_unexpected_content_type": "Codex/Cursor should target http://127.0.0.1:9449/mcp. The handler must return MCP JSON/SSE for every initialize request, including repeats.",
            "quickstart": "Run fighorse quickstart \"<figma-frame-url>\" for the shortest public onboarding path."
        },
        "proxy": {"configured": cfg.proxy.is_some(), "value": cfg.proxy},
        "recommended_next_step": if has_token { "Call list_experiences, then get_design_package with a Figma URL." } else { "Set FIGMA_TOKEN or run fighorse auth login --token <FIGMA_TOKEN>." }
    })
}

/// Build MCP client configuration.
pub fn mcp_config(client: &str, transport: &str, port: i64, command: &str) -> Value {
    let stdio = json!({
        "command": command,
        "args": ["mcp", "serve", "--transport", "stdio"],
        "env": {"FIGMA_TOKEN": "<FIGMA_TOKEN>", "FIGHORSE_HOME": "~/.fighorse", "FIGHORSE_MCP_MODE": "readonly", "FIGHORSE_MCP_LOCAL_WRITE": "allow"}
    });
    let sse = json!({
        "command": command,
        "args": ["mcp", "serve", "--transport", "sse", "--host", "127.0.0.1", "--port", port.to_string()],
        "url": format!("http://127.0.0.1:{port}/sse"),
        "env": {"FIGMA_TOKEN": "<FIGMA_TOKEN>", "FIGHORSE_HOME": "~/.fighorse", "FIGHORSE_MCP_MODE": "readonly", "FIGHORSE_MCP_LOCAL_WRITE": "allow"}
    });
    let http = json!({"transport": "http", "url": format!("http://127.0.0.1:{port}/mcp")});

    let config = match transport {
        "http" => http.clone(),
        "sse" => sse.clone(),
        _ => stdio.clone(),
    };
    let cursor_inner = match transport {
        "http" => json!({"url": http["url"]}),
        "sse" => json!({"url": sse["url"]}),
        _ => stdio.clone(),
    };
    let generic_inner = match transport {
        "http" => http.clone(),
        "sse" => sse.clone(),
        _ => stdio.clone(),
    };

    json!({
        "kind": "fighorse.mcp-config.v1",
        "client": client,
        "transport": transport,
        "recommended_tool_order": ["discover_fighorse", "check_fighorse_ready", "list_experiences", "get_design_package", "record_experience"],
        "config": config,
        "examples": {
            "cursor": {"mcpServers": {"fighorse": cursor_inner}},
            "generic": {"fighorse": generic_inner}
        }
    })
}

/// The full discovery manifest.
pub fn manifest() -> Value {
    let opts = experience::ScopeOpts::default();
    json!({
        "kind": "fighorse.discovery.v1",
        "name": "fighorse",
        "version": VERSION,
        "purpose": "Provide public-first Figma CLI + MCP infrastructure for design context, screenshots, tokens, assets, diagnostics, and implementation hints.",
        "primary_use_case": "Given a specific Figma frame URL, produce enough structured context for an AI coding tool to recreate the selected design.",
        "production_defaults": {
            "mcp_mode": "readonly",
            "mcp_local_write": "set FIGHORSE_MCP_LOCAL_WRITE=allow only for safe local asset exports",
            "fighorse_home": "~/.fighorse",
            "global_experience": "~/.fighorse/experience/global.jsonl",
            "project_experience": "./.fighorse/experience.jsonl after fighorse install project",
            "quickstart": "fighorse quickstart \"<figma-frame-url>\"",
            "auth_setup": "fighorse auth login --token <FIGMA_TOKEN>",
            "default_design_package": {"depth": 2, "max_tokens": 8000, "include_screenshot": true, "include_assets": false, "platform": "ask-developer-if-unspecified", "asset_format": "ask-developer-if-unspecified; png is only the render fallback"},
            "smoke_test": "fighorse smoke <figma-url>"
        },
        "setup_requirements": setup_guidance(),
        "input_contract": {
            "preferred": "figma_url",
            "accepted": ["Figma design/file/proto/board URL", "raw Figma file_key plus optional node_id", "optional target platform/framework, e.g. android-compose", "optional preferred asset format, e.g. png or svg"],
            "node_id_note": "Figma URLs use node-id=1-2. Figma REST APIs use 1:2. fighorse normalizes this automatically."
        },
        "output_contracts": {
            "design_package": {
                "kind": "fighorse.design-package.v1",
                "contains": ["source", "file", "target", "implementation_target", "screen_candidates", "component_candidates", "fidelity_workflow", "asset_export_plan", "learned_experience", "context", "tokens", "token_confidence", "missing_font_diagnostics", "screenshots", "assets", "implementation_risk_checklist", "implementation_hints"],
                "best_for": "AI design replication and implementation planning"
            },
            "experience_record": {
                "kind": experience::RECORD_KIND,
                "schema_version": experience::SCHEMA_VERSION,
                "best_for": "Persisting reusable lessons from real Figma replication, screenshot comparison, asset export, and platform debugging."
            }
        },
        "api_coverage": api_coverage::coverage_report(),
        "official_mcp_comparison": {
            "official_strengths": ["Native Figma canvas writes through official MCP product APIs.", "Code Connect-aware context and code generation inside Figma's product surface.", "Make resources, FigJam generation, and hosted Remote MCP ergonomics."],
            "fighorse_strengths": ["Self-hosted CLI-first pipeline under 1PL.", "Full public REST coverage with transparent operation registry.", "AI self-discovery, local experience learning, asset manifests, and reproducible visual feedback loops.", "Separate Figma write and local filesystem write safety controls."],
            "unsupported_by_public_rest": api_coverage::official_mcp_only_capabilities(),
            "recommended_setup": "Use both fighorse and the official Figma Remote MCP together. fighorse handles design-to-code read workflows; official MCP handles canvas writes, Code to Canvas, and Code Connect."
        },
        "complementary_mcp_servers": [{
            "name": "figma-official",
            "purpose": "Native canvas writes, Code to Canvas, Code Connect, and product-only Figma capabilities.",
            "remote_url": "https://mcp.figma.com/mcp",
            "transport": "http",
            "auth": "OAuth via Figma account",
            "pricing": "Beta: free. Future: usage-based paid feature (per Figma docs).",
            "seat_requirements": "Full seat for write to shared files; Dev seat read-only outside drafts.",
            "when_to_use": ["Write directly to Figma canvas", "Code to Canvas (push running UI into Figma as editable layers)", "Code Connect automatic mapping", "FigJam generation", "Make resources"],
            "when_not_to_use": "Design-to-code replication, asset export with manifests, visual audit, or local experience learning — use fighorse instead."
        }],
        "experience_loop": {
            "store_path": experience::experience_path(&opts).to_string_lossy(),
            "store": experience::store_info(&opts),
            "schema": "Call get_experience_schema or fighorse experience schema for the versioned JSONL contract.",
            "before_work": "Call list_experiences or fighorse experience summary with platform/asset_format filters before implementing.",
            "after_work": "Call record_experience or fighorse experience add whenever a reusable mismatch, asset rule, platform rule, or workflow fix is learned.",
            "compatibility": "Experience records are append-only JSONL. Readers must ignore unknown fields so future fighorse versions remain compatible."
        },
        "learned_experience": experience::guidance(&experience::Filters::default(), 5, &opts),
        "ai_contract": guidance::ai_contract(),
        "output_locations": guidance::output_location_guidance(),
        "recommended_workflow": [
            {"step": 1, "tool": "discover_fighorse", "reason": "Learn available tools and contracts without external instructions."},
            {"step": 2, "tool": "check_fighorse_ready", "reason": "Verify local setup. If auth.has_token is false, prompt the user to run fighorse auth login --token <FIGMA_TOKEN> before calling Figma APIs."},
            {"step": 3, "tool": "list_experiences", "reason": "Load reusable local lessons before repeating known layout, typography, asset, or platform mistakes."},
            {"step": 4, "tool": "parse_figma_url", "reason": "Extract file_key and node_id from a pasted Figma URL when needed."},
            {"step": 5, "tool": "get_design_package", "reason": "Fetch compact structure, screenshots, tokens, platform guidance, learned experience, asset export plan, and implementation hints in one call."},
            {"step": 6, "action": "If the target is a CANVAS/page/user flow or contains many children, narrow to exact frame/screen nodes before coding.", "reason": "Whole-flow pages are context for navigation, not a single UI surface to implement directly."},
            {"step": 7, "action": "Ask the developer for missing platform/framework or asset format before implementation.", "reason": "Platform and asset format change typography, density, vector/raster export, and build-pipeline choices."},
            {"step": 8, "action": "Export assets into a project-local or fighorse-managed directory with manifest enabled.", "reason": "Reasonable output locations avoid permission failures and make generated files discoverable by AI tools and build scripts."},
            {"step": 9, "action": "Implement from the design package.", "reason": "Use screenshots for visual fidelity, context for layout, tokens for styling, and assets for image fills."},
            {"step": 10, "action": "Run the implementation, capture screenshots, compare, and fix overlap/clipping/typography before finalizing.", "reason": "Real app screenshots catch container stacking, system chrome, compact typography, and localization issues."},
            {"step": 11, "tool": "visual_audit", "reason": "After implementation screenshots exist, structure fidelity checks and reusable experience suggestions."},
            {"step": 12, "tool": "record_experience", "reason": "Persist reusable lessons so the next AI client can self-learn from this run without a long prompt."}
        ],
        "mcp": {
            "transports": {
                "http": {"url": "http://127.0.0.1:9449/mcp", "requires": "Run the installed local service once; clients should reuse it instead of spawning stdio processes."},
                "stdio": {"command": "fighorse", "args": ["mcp", "serve", "--transport", "stdio"], "env": {"FIGHORSE_MCP_MODE": "readonly", "FIGHORSE_MCP_LOCAL_WRITE": "allow"}},
                "sse": {"command": "fighorse", "args": ["mcp", "serve", "--transport", "sse", "--host", "127.0.0.1", "--port", "9449"], "url": "http://127.0.0.1:9449/sse", "env": {"FIGHORSE_MCP_MODE": "readonly", "FIGHORSE_MCP_LOCAL_WRITE": "allow"}}
            },
            "local_write": {"env": "FIGHORSE_MCP_LOCAL_WRITE=allow", "allowed_roots": ["./.fighorse/exports", "./assets/fighorse", "~/.fighorse/exports"], "default": "deny unless enabled by install-generated MCP configs"},
            "default_mode": "readonly",
            "write_mode": "Set FIGHORSE_MCP_MODE=write only when the AI client is allowed to mutate Figma resources.",
            "self_discovery_tools": ["discover_fighorse", "check_fighorse_ready", "parse_figma_url", "get_replicate_workflow", "get_experience_schema", "list_experiences"],
            "learning_tools": ["get_experience_schema", "list_experiences", "record_experience"],
            "replication_tools": ["get_design_package", "get_design_context", "get_screenshot", "export_images", "export_component", "download_image_fills", "get_tokens", "visual_audit", "get_project_playbook"],
            "resources": ["fighorse://capabilities", "fighorse://coverage", "fighorse://workflow/design-replication", "fighorse://experience/summary"],
            "prompts": ["fighorse_design_replication", "fighorse_api_coverage"],
            "complementary_servers": [{"name": "figma-official", "url": "https://mcp.figma.com/mcp", "transport": "http", "auth": "OAuth", "purpose": "Canvas writes, Code to Canvas, Code Connect", "pricing_note": "Free during beta; will become usage-based paid"}]
        },
        "cli": {
            "self_discovery_commands": [
                "fighorse quickstart \"<figma-frame-url>\" --format json",
                "fighorse discover --format json",
                "fighorse doctor --format json",
                "fighorse install status",
                "fighorse install project",
                "fighorse smoke <figma-url>",
                "fighorse url parse <figma-url>",
                "fighorse experience summary --platform <target-platform> --asset-format <asset-format>",
                "fighorse experience add --summary <issue-pattern> --lesson <generalized-lesson> --platform <target-platform> --asset-format <asset-format>",
                "fighorse design package <figma-url> --platform <target-platform> --asset-format <asset-format> --max-tokens 8000",
                "fighorse mcp config --client cursor --transport http"
            ],
            "install_commands": [
                "fighorse install home",
                "fighorse install auth --apply",
                "fighorse install binary --source <path-to-fighorse-binary> --apply",
                "fighorse install project",
                "fighorse install client --client cursor",
                "fighorse install client --client cursor --apply",
                "fighorse install client --client codex",
                "fighorse install client --client codex --apply",
                "fighorse install client --client kimi --apply",
                "fighorse install client --client claude",
                "fighorse install client --client opencode",
                "fighorse install service --service launchd --apply",
                "fighorse install skill --clients cursor,codex,kimi --apply",
                "fighorse install --default --apply",
                "fighorse install --path ~/.local/bin --apply",
                "fighorse install --default --mode service --clients cursor,codex,kimi --apply"
            ]
        },
        "auth": {
            "required_for_figma_api": true,
            "env": ["FIGMA_TOKEN", "FIGMA_API_KEY"],
            "local_config": "fighorse auth login --token <FIGMA_TOKEN>",
            "missing_token_ai_prompt": "fighorse needs a Figma Personal Access Token before it can read Figma files. Please run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then ask me to retry.",
            "safety": "Do not commit tokens. Pass tokens through environment variables or local config only."
        },
        "quality_rules": [
            "Use screenshot output as the visual source of truth.",
            "Before implementation, review relevant local lessons with list_experiences or fighorse experience summary.",
            "Before implementation, know the target platform/framework and asset format; if not supplied, ask the developer instead of guessing.",
            "Use compact context for hierarchy, layout, spacing, text, and component clues.",
            "Use tokens for colors, typography, spacing, and shadows.",
            "Map typography explicitly: font family, font size, font weight, line height, and letter spacing.",
            "When implementation details conflict, prioritize screenshots, then explicit tokens, then compact tree metadata.",
            "If a required asset URL is missing, call get_image_fills, export_images, export_component, download_image_fills, image export, component export, or asset download before guessing.",
            "Use --manifest for exported slices/assets when another AI tool or build script needs to discover generated files without extra instructions.",
            "Store exports in ./.fighorse/exports for scratch work, ./assets/fighorse or the app resource directory for packaged assets, or ~/.fighorse/exports for cross-project scratch data.",
            "MCP export tools require FIGHORSE_MCP_LOCAL_WRITE=allow and reject paths outside ./.fighorse/exports, ./assets/fighorse, and ~/.fighorse/exports.",
            "Do not write generated exports to protected system paths, dependency caches, or hard-to-discover temp locations unless the developer explicitly asks.",
            "Use a visual debug loop: implement, build/run, capture screenshot, compare with Figma, then fix overlap, clipping, status bars, and compact typography.",
            "Inspect repeated components and child nodes individually when a whole-screen package is ambiguous.",
            "If the selected target is a CANVAS, page, user flow, or contains many screen children, ask for the exact frame/screen or inspect the tree before implementing.",
            "After fixing a reusable mismatch, record it with record_experience or fighorse experience add so future runs learn automatically."
        ]
    })
}

/// The replication workflow payload.
pub fn workflow() -> Value {
    json!({
        "kind": "fighorse.replicate-workflow.v1",
        "goal": "Recreate a Figma design in code with high visual fidelity.",
        "default_call": {
            "tool": "get_design_package",
            "arguments": {"figma_url": "<paste Figma URL>", "platform": "<target platform/framework, ask developer if unknown>", "asset_format": "<asset format, ask developer if unknown>", "depth": 2, "max_tokens": 8000, "include_screenshot": true, "include_assets": true}
        },
        "steps": [
            "Call list_experiences with platform/asset_format filters to load local lessons before implementing.",
            "Call get_design_package with the pasted Figma URL.",
            "If platform/framework or asset format is unknown, ask the developer before choosing implementation rules; png is only a render fallback, not a silent product decision.",
            "Inspect target, screenshots, context, tokens, and assets.",
            "Inspect important child nodes/components individually when layout, typography, or asset treatment is ambiguous.",
            "Export local slices/components with manifest=true for icons, image fills, controls, and ambiguous visual details into ./.fighorse/exports, ./assets/fighorse, or another developer-approved directory.",
            "Map Figma frames to app components and layout containers.",
            "Implement styling from tokens and compact tree dimensions/layout.",
            "Use screenshots to compare spacing, typography, colors, and hierarchy.",
            "Run the project's normal tests/build after implementation.",
            "When possible, run the app, capture screenshots, compare against Figma, and fix overlap/clipping/typography before finalizing.",
            "Call record_experience after a reusable lesson is discovered so the next run can self-learn."
        ],
        "avoid": [
            "Do not ask the user to manually extract file_key or node_id from a normal Figma URL.",
            "Do not choose platform/framework or asset format silently when the developer has not specified it.",
            "Do not ignore screenshots when they are available.",
            "Do not invent image assets when get_image_fills or screenshots provide references.",
            "Do not write exports to protected system paths, dependency caches, or hidden locations that the app/build cannot easily use.",
            "Do not assume a single typography scale applies to compact and full-size components.",
            "Do not discard lessons from screenshot debugging; persist reusable findings through the experience interface."
        ]
    })
}

/// Markdown rendering of the manifest.
pub fn manifest_markdown(m: &Value) -> String {
    let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let version = m.get("version").and_then(|v| v.as_str()).unwrap_or("");
    let purpose = m.get("purpose").and_then(|v| v.as_str()).unwrap_or("");
    let primary = m.get("primary_use_case").and_then(|v| v.as_str()).unwrap_or("");

    let workflow_lines: Vec<String> = m
        .get("recommended_workflow")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|w| {
                    let step = w.get("step").and_then(|v| v.as_i64()).unwrap_or(0);
                    let label = w
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .or_else(|| w.get("action").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let reason = w.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                    format!("{step}. {label} - {reason}")
                })
                .collect()
        })
        .unwrap_or_default();

    let cli_lines: Vec<String> = m
        .get("cli")
        .and_then(|c| c.get("self_discovery_commands"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| format!("- `{}`", c.as_str().unwrap_or("")))
                .collect()
        })
        .unwrap_or_default();

    format!(
        "# {name} {version}\n\n{purpose}\n\n## Primary Use Case\n\n{primary}\n\n## Setup Requirements\n\n\
- Figma API calls require a Figma Personal Access Token.\n\
- Recommended setup: `fighorse auth login --token <FIGMA_TOKEN>`.\n\
- One-shot setup: `FIGMA_TOKEN=<FIGMA_TOKEN> fighorse quickstart \"<figma-frame-url>\"`.\n\
- If `check_fighorse_ready` reports `auth.has_token=false`, ask the user to configure the token before calling Figma API tools.\n\n\
## Recommended Workflow\n\n{}\n\n## CLI Self Discovery\n\n{}\n\n## Experience Loop\n\n\
Before implementation, run `fighorse experience summary --platform <platform> --asset-format <format>`. After fixing a reusable mismatch, run `fighorse experience add --summary <issue> --lesson <lesson> --platform <platform>`.\n\n",
        workflow_lines.join("\n"),
        cli_lines.join("\n")
    )
}
