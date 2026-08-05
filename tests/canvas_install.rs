use fighorse::install;
use fighorse::install::service;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("fighorse-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn service_templates_keep_canvas_closed_by_default() {
    let launchd = service::launchd_plist("/bin/fighorse", 9449, "/tmp/fighorse", false);
    assert!(launchd.contains("FIGHORSE_CANVAS_MODE"));
    assert!(launchd.contains("<key>FIGHORSE_CANVAS_MODE</key><string>readonly</string>"));
    assert!(launchd.contains("<key>FIGHORSE_CANVAS_SCRIPT</key><string>deny</string>"));
    assert!(launchd.contains("<key>FIGHORSE_CANVAS_BRIDGE</key><string>deny</string>"));

    let systemd = service::systemd_unit("/bin/fighorse", 9449, "/tmp/fighorse", false);
    assert!(systemd.contains("Environment=\"FIGHORSE_CANVAS_MODE=readonly\""));
    assert!(systemd.contains("Environment=\"FIGHORSE_CANVAS_SCRIPT=deny\""));
    assert!(systemd.contains("Environment=\"FIGHORSE_CANVAS_BRIDGE=deny\""));
}

#[test]
fn service_templates_can_enable_canvas_bridge_explicitly() {
    let systemd = service::systemd_unit_with_canvas(
        "/bin/fighorse",
        9449,
        "/tmp/fighorse",
        false,
        service::CanvasServiceConfig {
            mode: "write",
            script: "allow",
            bridge: "allow",
            port: 9450,
        },
    );
    assert!(systemd.contains("Environment=\"FIGHORSE_MCP_MODE=write\""));
    assert!(systemd.contains("Environment=\"FIGHORSE_CANVAS_MODE=write\""));
    assert!(systemd.contains("Environment=\"FIGHORSE_CANVAS_SCRIPT=allow\""));
    assert!(systemd.contains("Environment=\"FIGHORSE_CANVAS_BRIDGE=allow\""));
    assert!(systemd.contains("Environment=\"FIGHORSE_CANVAS_PORT=9450\""));
}

#[test]
fn canvas_plugin_install_writes_managed_bundle() {
    let root = temp_root("canvas-plugin-install");
    let home = root.to_string_lossy().to_string();

    let report = install::install_canvas_plugin(Some(&home), true).unwrap();
    assert_eq!(report["apply"], true);
    assert!(root.join("plugins/fighorse-canvas/manifest.json").is_file());
    assert!(root.join("plugins/fighorse-canvas/code.js").is_file());
    assert!(root.join("install/manifest.json").is_file());

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(root.join("install/manifest.json")).unwrap())
            .unwrap();
    let managed = manifest["managed_files"].as_array().unwrap();
    assert!(managed.iter().any(|file| {
        file["path"]
            .as_str()
            .unwrap()
            .ends_with("plugins/fighorse-canvas/manifest.json")
    }));

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn service_canvas_plugin_uses_configured_bridge_port() {
    let root = temp_root("canvas-plugin-port");
    let home = root.join("home").to_string_lossy().to_string();

    install::install_all_async(&install::InstallOpts {
        source: None,
        path: None,
        target: None,
        default: false,
        client: None,
        clients: None,
        transport: "http",
        port: 9449,
        command: "/bin/fighorse",
        home: Some(&home),
        token: None,
        mode: Some("service"),
        service: "none",
        link_dir: None,
        link_dirs: None,
        no_service: false,
        canvas_plugin: true,
        canvas_mode: Some("write"),
        canvas_script: Some("deny"),
        canvas_port: Some(9560),
        apply: true,
    })
    .await
    .unwrap();

    let manifest =
        fs::read_to_string(root.join("home/plugins/fighorse-canvas/manifest.json")).unwrap();
    let ui = fs::read_to_string(root.join("home/plugins/fighorse-canvas/ui.html")).unwrap();
    assert!(manifest.contains("127.0.0.1:9560"));
    assert!(ui.contains("127.0.0.1:9560"));

    let _ = fs::remove_dir_all(root);
}
