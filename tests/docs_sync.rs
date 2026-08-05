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
        assert!(
            text.contains("discover_fighorse")
                && text.contains("get_resource_catalog")
                && text.contains("Codex"),
            "{path} omits a Codex read-only bootstrap approval boundary"
        );
        assert!(
            !text.contains("default_tools_approval_mode"),
            "{path} must not recommend approving every Codex MCP tool"
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
fn ai_plugin_bundle_contract_is_synchronized_across_surfaces() {
    for path in [
        "src/main.rs",
        "src/discovery.rs",
        "src/install.rs",
        "src/install/skill.md",
        "src/install/agents.md",
        "docs/specs/ai-plugin-bundle-contract.md",
    ] {
        let text = read(path);
        for token in [
            "install ai-plugin",
            "ai-plugin/fighorse",
            "fighorse-canvas-write",
            "http://127.0.0.1:9449/mcp",
        ] {
            assert!(text.contains(token), "{path} omits AI plugin token {token}");
        }
    }

    let mut docs = vec![("README.md".to_string(), read("README.md"))];
    docs.extend(localized("README.md"));
    for name in ["user-guide.md", "ai-client-guide.md", "design.md"] {
        docs.extend(localized(name));
    }
    for (path, text) in docs {
        assert!(
            text.contains("install ai-plugin"),
            "{path} omits install ai-plugin"
        );
        assert!(
            text.contains("ai-plugin/fighorse"),
            "{path} omits ai plugin package path"
        );
        assert!(
            text.contains("fighorse-canvas-write"),
            "{path} omits workflow skills"
        );
        assert!(
            text.contains("Verified by Cursor")
                || text.contains("local-only")
                || text.contains("本地-only"),
            "{path} omits local-only / not-verified boundary"
        );
    }
}

#[test]
fn resource_catalog_contract_is_synchronized_across_user_and_ai_surfaces() {
    let mut readmes = vec![("README.md".to_string(), read("README.md"))];
    readmes.extend(localized("README.md"));
    for (path, text) in readmes {
        assert!(
            text.contains("resource catalog")
                || text.contains("资源目录")
                || text.contains("каталог"),
            "{path} omits the resource catalog command"
        );
        assert!(
            text.contains("get_resource_catalog"),
            "{path} omits the MCP catalog tool"
        );
    }

    for name in ["user-guide.md", "ai-client-guide.md", "design.md"] {
        for (path, text) in localized(name) {
            assert!(
                text.contains("get_resource_catalog") || name == "design.md",
                "{path} omits the shared catalog entry point"
            );
            if name != "design.md" {
                assert!(
                    text.contains("partial") && text.contains("blocked"),
                    "{path} omits catalog status behavior"
                );
            }
        }
    }
    for (path, text) in localized("design.md") {
        assert!(
            text.contains("50"),
            "{path} has not been calibrated to the current operation registry"
        );
    }

    for path in [
        "src/main.rs",
        "src/discovery.rs",
        "src/guidance.rs",
        "src/install.rs",
        "src/install/skill.md",
        "src/install/agents.md",
        "src/mcp/tools_extra.json",
        "docs/specs/resource-catalog-contract.md",
        ".agents/experience/figma-browser-url-boundaries.md",
    ] {
        let text = read(path);
        assert!(
            text.contains("get_resource_catalog") || text.contains("resource catalog"),
            "{path} omits the resource catalog contract"
        );
    }
}

#[test]
fn routing_and_design_scope_contracts_are_synchronized() {
    for path in [
        "src/url.rs",
        "src/mcp/tools_extra.json",
        "src/discovery.rs",
        "src/guidance.rs",
        "src/main.rs",
        "src/install/skill.md",
        "src/install/agents.md",
        "docs/specs/figma-url-routing-contract.md",
        ".agents/experience/figma-browser-url-boundaries.md",
    ] {
        let text = read(path);
        for token in ["url_role", "catalog_eligible", "next_action"] {
            assert!(
                text.contains(token),
                "{path} omits URL routing token {token}"
            );
        }
        assert!(
            text.contains("browser_root_not_enumerable"),
            "{path} omits browser-root routing error code"
        );
    }

    for path in [
        "src/product/design_package.rs",
        "src/mcp/tools_extra.json",
        "src/discovery.rs",
        "src/guidance.rs",
        "src/install/skill.md",
        "src/install/agents.md",
        "docs/specs/design-package-contract.md",
    ] {
        let text = read(path);
        for token in ["scope", "needs_narrowing", "implementable", "null_count"] {
            assert!(
                text.contains(token),
                "{path} omits design scope token {token}"
            );
        }
        assert!(
            text.contains("SECTION"),
            "{path} omits SECTION narrowing guidance"
        );
    }

    let mut user_docs = vec![("README.md".to_string(), read("README.md"))];
    for name in [
        "README.md",
        "user-guide.md",
        "ai-client-guide.md",
        "design.md",
    ] {
        user_docs.extend(localized(name));
    }
    for (path, text) in user_docs {
        assert!(
            text.contains("catalog_eligible") && text.contains("browser_root_not_enumerable"),
            "{path} omits machine-readable URL routing guidance"
        );
        assert!(
            text.contains("needs_narrowing") && text.contains("SECTION"),
            "{path} omits container narrowing guidance"
        );
    }
}

#[test]
fn canvas_bridge_contract_is_synchronized_across_user_and_ai_surfaces() {
    for path in [
        "README.md",
        "src/main.rs",
        "src/discovery.rs",
        "src/guidance.rs",
        "src/install.rs",
        "src/install/skill.md",
        "src/install/agents.md",
        "src/mcp/tools_write.json",
        "docs/specs/canvas-write-contract.md",
        ".agents/experience/canvas-plugin-boundaries.md",
    ] {
        let text = read(path);
        for token in [
            "FIGHORSE_CANVAS_MODE",
            "FIGHORSE_CANVAS_SCRIPT",
            "canvas_execute_script",
        ] {
            assert!(text.contains(token), "{path} omits canvas token {token}");
        }
    }

    let extra = read("src/mcp/tools_extra.json");
    assert!(extra.contains("canvas_status"));
    assert!(extra.contains("canvas_create_pairing"));

    for name in [
        "README.md",
        "user-guide.md",
        "ai-client-guide.md",
        "design.md",
    ] {
        for (path, text) in localized(name) {
            assert!(
                text.contains("canvas") || text.contains("Canvas"),
                "{path} omits canvas bridge guidance"
            );
            assert!(
                text.contains("FIGHORSE_CANVAS_MODE"),
                "{path} omits canvas mode guidance"
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
    for (path, text) in [
        ("src/discovery.rs", discovery.as_str()),
        ("src/install/skill.md", skill.as_str()),
        ("src/install/agents.md", agents.as_str()),
        ("src/main.rs", sources[0].1.as_str()),
    ] {
        assert!(
            text.contains("projects:read") && text.contains("team"),
            "{path} omits Figma team-browser permission guidance"
        );
    }
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
