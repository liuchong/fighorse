use std::fs;
use std::path::{Path, PathBuf};

const FOUR_CLIENTS: &str = "cursor,codex,kimi,claude";
const ENDPOINT: &str = "http://127.0.0.1:9449/mcp";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: impl AsRef<Path>) -> String {
    let path = root().join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn localized(name: &str) -> Vec<(String, String)> {
    ["en", "zh", "ru"]
        .into_iter()
        .map(|language| {
            let path = format!("docs/{language}/{name}");
            (path.clone(), read(path))
        })
        .collect()
}

#[test]
fn readmes_publish_one_four_client_service_workflow() {
    let mut files = vec![("README.md".to_string(), read("README.md"))];
    files.extend(localized("README.md"));

    for (path, text) in files {
        assert!(
            text.contains(FOUR_CLIENTS),
            "{path} omits a canonical client"
        );
        assert!(text.contains(ENDPOINT), "{path} omits the /mcp endpoint");
        assert!(
            text.contains("install client --client claude --apply"),
            "{path} omits the standalone Claude install command"
        );
        assert!(
            text.contains("install verify") && text.contains("install rollback"),
            "{path} omits transaction verification or rollback"
        );
        assert!(
            text.contains("code-connect generate") && text.contains("FIGHORSE_MCP_CODE_CONNECT"),
            "{path} omits Code Connect commands or safety defaults"
        );
    }
}

#[test]
fn localized_guides_cover_payloads_transaction_and_skill_targets() {
    for name in [
        "quickstart.md",
        "user-guide.md",
        "design.md",
        "ai-client-guide.md",
    ] {
        for (path, text) in localized(name) {
            assert!(text.contains(ENDPOINT), "{path} omits the /mcp endpoint");
            assert!(
                text.contains("~/.agents/skills/fighorse/SKILL.md"),
                "{path} omits the shared skill target"
            );
            assert!(
                text.contains("~/.claude/skills/fighorse/SKILL.md"),
                "{path} omits the Claude skill target"
            );
            assert!(
                text.contains("~/.cursor/rules/fighorse.mdc"),
                "{path} omits the Cursor rule target"
            );
        }
    }

    for name in ["user-guide.md", "design.md", "ai-client-guide.md"] {
        for (path, text) in localized(name) {
            assert!(
                text.contains("Code Connect") && text.contains("FIGHORSE_MCP_CODE_CONNECT"),
                "{path} omits Code Connect safety guidance"
            );
            for payload_token in [
                r#""url""#,
                r#""transport":"http""#,
                r#""type":"http""#,
                "[mcp_servers.fighorse]",
            ] {
                assert!(
                    text.replace(' ', "").contains(payload_token),
                    "{path} omits payload token {payload_token}"
                );
            }
            assert!(
                text.contains("desired_absent"),
                "{path} omits managed removal semantics"
            );
        }
    }
}

#[test]
fn docs_distinguish_streamable_http_event_streams_from_legacy_sse() {
    let mut paths = vec!["README.md".to_string()];
    for language in ["en", "zh", "ru"] {
        for name in [
            "README.md",
            "quickstart.md",
            "user-guide.md",
            "design.md",
            "ai-client-guide.md",
        ] {
            paths.push(format!("docs/{language}/{name}"));
        }
    }

    for path in paths {
        let text = read(&path);
        for stale in [
            "StreamableHTTPServerTransport",
            "stateless per request",
            "stateless-per-request",
            "FIGHORSE_CLI_EXPLICIT_EXIT",
            "FIGHORSE_MCP_STDIO_POLL",
            "FIGHORSE_STDIO_POLL",
            "8 MB frame",
            "Bun.serve",
        ] {
            assert!(!text.contains(stale), "{path} still contains {stale}");
        }
        assert!(
            !text.contains("http://127.0.0.1:9449/sse"),
            "{path} advertises the retired /sse endpoint"
        );
        assert!(
            !text.contains("http://127.0.0.1:9449/messages"),
            "{path} advertises the retired /messages endpoint"
        );
    }

    for (path, text) in localized("design.md") {
        assert!(
            text.contains("rmcp") && text.contains("stateful"),
            "{path} does not describe the official stateful rmcp service"
        );
        assert!(
            text.contains("Origin") && text.contains("Host"),
            "{path} omits HTTP Origin/Host validation"
        );
        assert!(text.contains("graceful"), "{path} omits graceful shutdown");
        assert!(
            text.contains("JSON") && text.contains("event-stream"),
            "{path} omits Streamable HTTP response negotiation"
        );
    }
}

#[test]
fn ai_facing_sources_use_four_clients_and_canonical_targets() {
    let sources = [
        ("src/main.rs", read("src/main.rs")),
        ("src/discovery.rs", read("src/discovery.rs")),
        ("src/install.rs", read("src/install.rs")),
        ("src/install/skill.md", read("src/install/skill.md")),
        ("src/install/agents.md", read("src/install/agents.md")),
        ("src/experience.rs", read("src/experience.rs")),
        ("AGENTS.md", read("AGENTS.md")),
    ];
    for (path, text) in &sources {
        assert!(
            !text.contains("cursor,codex,kimi --apply"),
            "{path} has a successful three-client command"
        );
        assert!(
            !text.contains("FIGHORSE_CLI_EXPLICIT_EXIT")
                && !text.contains("FIGHORSE_MCP_STDIO_POLL")
                && !text.contains("FIGHORSE_STDIO_POLL"),
            "{path} retains a removed runtime workaround"
        );
    }

    let discovery = read("src/discovery.rs");
    let install = read("src/install.rs");
    let skill = read("src/install/skill.md");
    let agents = read("src/install/agents.md");
    assert!(discovery.contains(FOUR_CLIENTS));
    assert!(install.contains(FOUR_CLIENTS));
    assert!(discovery.contains("FIGHORSE_MCP_CODE_CONNECT"));
    assert!(install.contains("FIGHORSE_MCP_CODE_CONNECT"));
    assert!(skill.contains("FIGHORSE_MCP_CODE_CONNECT"));
    assert!(agents.contains("FIGHORSE_MCP_CODE_CONNECT"));
    assert!(skill.contains("reconnect the MCP client"));
    assert!(skill.contains("tools/list"));
    assert!(agents.contains("reconnect the MCP client"));
    assert!(agents.contains("tools/list"));
    assert!(read("src/main.rs").contains("cursor|codex|kimi|claude"));
    assert!(read("src/experience.rs").contains("cursor|codex|kimi|claude"));
    for target in [
        ".agents/skills/fighorse/SKILL.md",
        ".claude/skills/fighorse/SKILL.md",
        ".cursor/rules/fighorse.mdc",
    ] {
        assert!(discovery.contains(target), "discovery omits {target}");
        assert!(install.contains(target), "install output omits {target}");
    }
}

#[test]
fn private_compatibility_renderers_do_not_accept_sse() {
    let install = read("src/install.rs");
    for stale in [
        r#"Some("http" | "sse")"#,
        r#"Some(t @ ("http" | "sse"))"#,
        "HTTP/SSE ->",
        "expects just {url} for HTTP/SSE",
    ] {
        assert!(
            !install.contains(stale),
            "src/install.rs still renders SSE through a private payload path: {stale}"
        );
    }
    assert!(
        install.contains("Legacy SSE transport is retired"),
        "the migration error must remain actionable"
    );
}
