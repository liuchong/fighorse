use fighorse::install::clients::{ClientKind, ClientSpec};
use fighorse::install::service::{launchd_plist, systemd_unit};
use fighorse::install::skills::canonical_targets;
use serde_json::json;
use std::path::Path;

const ENDPOINT: &str = "http://127.0.0.1:9449/mcp";

#[test]
fn four_clients_render_native_http_payloads() {
    assert_eq!(
        ClientSpec::new(ClientKind::Cursor, ENDPOINT).json_payload(),
        json!({"url": ENDPOINT})
    );
    assert_eq!(
        ClientSpec::new(ClientKind::Kimi, ENDPOINT).json_payload(),
        json!({"transport": "http", "url": ENDPOINT})
    );
    assert_eq!(
        ClientSpec::new(ClientKind::Claude, ENDPOINT).json_payload(),
        json!({"type": "http", "url": ENDPOINT})
    );
    let codex = ClientSpec::new(ClientKind::Codex, ENDPOINT).toml_payload();
    assert!(codex.contains(&format!("url = \"{ENDPOINT}\"")));
    assert!(codex.contains("[mcp_servers.fighorse.tools.discover_fighorse]"));
    assert!(codex.contains("approval_mode = \"approve\""));
    assert!(!codex.contains("default_tools_approval_mode"));
}

#[test]
fn service_units_use_streamable_http_and_deny_new_local_writes() {
    let launchd = launchd_plist("/tmp/fighorse", 9449, "/tmp/home", false);
    let systemd = systemd_unit("/tmp/fighorse", 9449, "/tmp/home", false);
    for rendered in [&launchd, &systemd] {
        assert!(rendered.contains("--transport"));
        assert!(rendered.contains("http"));
        assert!(!rendered.contains("--transport</string><string>sse"));
        assert!(!rendered.contains("--transport sse"));
        assert!(rendered.contains("FIGHORSE_MCP_LOCAL_WRITE"));
        assert!(rendered.contains("FIGHORSE_MCP_SERVICE"));
        assert!(rendered.contains("deny"));
    }
    assert!(launchd.contains("<key>WorkingDirectory</key><string>/tmp/home</string>"));
}

#[test]
fn skills_have_only_canonical_cross_client_targets() {
    let targets = canonical_targets(
        Path::new("/home/test"),
        &[
            ClientKind::Cursor,
            ClientKind::Kimi,
            ClientKind::Claude,
            ClientKind::Codex,
        ],
    );
    let paths: Vec<_> = targets.iter().map(|target| target.path.as_path()).collect();
    assert!(paths.contains(&Path::new("/home/test/.agents/skills/fighorse/SKILL.md")));
    assert!(paths.contains(&Path::new("/home/test/.claude/skills/fighorse/SKILL.md")));
    assert!(paths.contains(&Path::new("/home/test/.cursor/rules/fighorse.mdc")));
    assert_eq!(paths.len(), 3);
}

#[test]
fn codex_merge_adds_discovery_approval_migrates_legacy_and_rejects_unknown() {
    let spec = ClientSpec::new(ClientKind::Codex, ENDPOINT);
    let equivalent = format!("model = \"gpt\"\n\n[mcp_servers.fighorse]\nurl = \"{ENDPOINT}\"\n");
    let migrated_equivalent = spec.merge_config(Some(&equivalent)).unwrap();
    assert!(migrated_equivalent.contains("# BEGIN fighorse managed"));
    assert!(migrated_equivalent.contains("[mcp_servers.fighorse.tools.discover_fighorse]"));
    assert!(migrated_equivalent.contains("approval_mode = \"approve\""));

    let approved = format!(
        "[mcp_servers.fighorse]\nurl = \"{ENDPOINT}\"\n\n[mcp_servers.fighorse.tools.discover_fighorse]\napproval_mode = \"approve\"\n"
    );
    assert_eq!(spec.merge_config(Some(&approved)).unwrap(), approved);

    let legacy = "model = \"gpt\"\n\n[mcp_servers.fighorse]\nurl = \"http://127.0.0.1:9449/sse\"\n";
    let migrated = spec.merge_config(Some(legacy)).unwrap();
    assert!(migrated.contains("model = \"gpt\""));
    assert!(migrated.contains("# BEGIN fighorse managed"));
    assert!(migrated.contains(&format!("url = \"{ENDPOINT}\"")));
    assert!(!migrated.contains("/sse"));

    for customized in [
        format!("[mcp_servers.fighorse]\nurl = \"{ENDPOINT}\"\ntool_timeout_sec = 120\n"),
        "[mcp_servers.fighorse]\nurl = \"http://127.0.0.1:9449/sse\"\ntool_timeout_sec = 120\n"
            .to_string(),
        format!(
            "[mcp_servers.fighorse]\nurl = \"{ENDPOINT}\"\n\n[mcp_servers.fighorse.tools.discover_fighorse]\napproval_mode = \"prompt\"\n"
        ),
        format!(
            "[mcp_servers.fighorse]\nurl = \"{ENDPOINT}\"\n\n[mcp_servers.fighorse.tools.get_file]\napproval_mode = \"approve\"\n"
        ),
    ] {
        let error = spec
            .merge_config(Some(&customized))
            .unwrap_err()
            .to_string();
        assert!(error.contains("user-managed"), "{error}");
    }

    let custom = "[mcp_servers.fighorse]\nurl = \"https://mcp.example.test/custom-fighorse\"\n";
    let error = spec.merge_config(Some(custom)).unwrap_err().to_string();
    assert!(error.contains("user-managed"), "{error}");
}
