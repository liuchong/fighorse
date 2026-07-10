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
    assert!(ClientSpec::new(ClientKind::Codex, ENDPOINT)
        .toml_payload()
        .contains(&format!("url = \"{ENDPOINT}\"")));
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
