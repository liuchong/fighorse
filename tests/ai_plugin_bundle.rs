use fighorse::install::{self, AiPluginInstallOpts};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const ENDPOINT: &str = "http://127.0.0.1:9449/mcp";

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "fighorse-ai-plugin-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn file_entry<'a>(files: &'a [Value], suffix: &str) -> &'a Value {
    files
        .iter()
        .find(|file| file["path"].as_str().unwrap().ends_with(suffix))
        .unwrap_or_else(|| panic!("missing bundle file ending with {suffix}"))
}

#[test]
fn ai_plugin_bundle_renders_shared_manifests_and_skills() {
    let bundle =
        install::ai_plugin_bundle(ENDPOINT, "cursor,codex,kimi,claude,opencode,gemini").unwrap();
    assert_eq!(bundle["publish_status"], "local_only");
    assert_eq!(bundle["mcp_endpoint"], ENDPOINT);
    assert_eq!(bundle["rmcp"]["decision"], "defer");
    assert!(
        bundle["clients"]
            .as_array()
            .unwrap()
            .contains(&Value::String("opencode".into()))
    );
    assert!(
        bundle["clients"]
            .as_array()
            .unwrap()
            .contains(&Value::String("gemini".into()))
    );

    let files = bundle["files"].as_array().unwrap();
    file_entry(files, ".cursor-plugin/plugin.json");
    file_entry(files, ".claude-plugin/plugin.json");
    file_entry(files, "gemini-extension.json");
    file_entry(files, "server.json");
    file_entry(files, ".mcp.json");
    for skill in [
        "skills/fighorse/SKILL.md",
        "skills/fighorse-design-to-code/SKILL.md",
        "skills/fighorse-canvas-write/SKILL.md",
        "skills/fighorse-resource-catalog/SKILL.md",
        "skills/fighorse-code-connect/SKILL.md",
        "skills/fighorse-self-learning/SKILL.md",
    ] {
        file_entry(files, skill);
    }

    let cursor: Value = serde_json::from_str(
        file_entry(files, ".cursor-plugin/plugin.json")["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(cursor["skills"], "./skills/");
    assert_eq!(cursor["mcpServers"], "./.mcp.json");
    assert_ne!(cursor["displayName"], "Figma");

    let mcp: Value =
        serde_json::from_str(file_entry(files, ".mcp.json")["content"].as_str().unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["fighorse"]["type"], "http");
    assert_eq!(mcp["mcpServers"]["fighorse"]["url"], ENDPOINT);
    assert!(
        mcp["mcpServers"]["fighorse"]["_meta"]["ideToolTitles"]["discover_fighorse"].is_string()
    );
    let serialized = serde_json::to_string(&mcp).unwrap();
    assert!(!serialized.contains("FIGMA_TOKEN"));
    assert!(!serialized.contains("allow"));
}

#[test]
fn ai_plugin_install_writes_bundle_into_managed_home() {
    let root = temp_root("install");
    let home = root.join("home").to_string_lossy().to_string();
    let report = install::install_ai_plugin(AiPluginInstallOpts {
        home: Some(&home),
        clients: Some("cursor,codex,kimi,claude,opencode,gemini"),
        endpoint: Some(ENDPOINT),
        apply: true,
    })
    .unwrap();

    assert_eq!(report["apply"], true);
    assert_eq!(report["bundle"]["publish_status"], "local_only");
    assert!(
        root.join("home/ai-plugin/fighorse/.cursor-plugin/plugin.json")
            .is_file()
    );
    assert!(root.join("home/ai-plugin/fighorse/.mcp.json").is_file());
    assert!(
        root.join("home/ai-plugin/fighorse/skills/fighorse/SKILL.md")
            .is_file()
    );
    assert!(root.join("home/install/manifest.json").is_file());
    let plugin =
        fs::read_to_string(root.join("home/ai-plugin/fighorse/.cursor-plugin/plugin.json"))
            .unwrap();
    assert!(plugin.contains("\"fighorse\""));
    assert!(!plugin.contains("Verified by Cursor"));

    let _ = fs::remove_dir_all(root);
}
