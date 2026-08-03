use fighorse::discovery;
use fighorse::mcp::tools;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn process_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

struct EnvGuard(Vec<(&'static str, Option<String>)>);

impl EnvGuard {
    fn capture(keys: &[&'static str]) -> Self {
        Self(
            keys.iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect(),
        )
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

fn tool_names() -> HashSet<String> {
    tools::list_tools()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(String::from))
        .collect()
}

#[test]
fn quickstart_keeps_cli_readiness_separate_from_optional_service_readiness() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&[
        "FIGHORSE_HOME",
        "FIGHORSE_MCP_LOCK_FILE",
        "FIGMA_TOKEN",
        "FIGMA_API_KEY",
    ]);
    let root = temp_root("quickstart-discovery");
    let lock_path = root.join("isolated-mcp.lock");
    unsafe { std::env::set_var("FIGHORSE_HOME", &root) };
    unsafe { std::env::set_var("FIGHORSE_MCP_LOCK_FILE", &lock_path) };
    unsafe { std::env::remove_var("FIGMA_TOKEN") };
    unsafe { std::env::remove_var("FIGMA_API_KEY") };

    let report = discovery::quickstart(None);
    assert_eq!(report["install"]["default_mode"], "cli");
    assert_eq!(report["mcp"]["owner_active"], false);
    assert_eq!(report["mcp"]["ready"], false);
    assert_eq!(report["mcp"]["required_for_cli"], false);
    assert!(
        report["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str())
            .any(|step| step.contains("cursor,codex,kimi,claude")),
        "{report}"
    );
    assert!(!lock_path.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn discovery_versions_follow_crate_version() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&["FIGHORSE_HOME", "FIGMA_TOKEN", "FIGMA_API_KEY"]);
    let root = temp_root("version-discovery");
    unsafe { std::env::set_var("FIGHORSE_HOME", &root) };
    unsafe { std::env::remove_var("FIGMA_TOKEN") };
    unsafe { std::env::remove_var("FIGMA_API_KEY") };

    let manifest = discovery::manifest();
    let doctor = discovery::doctor();
    assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(doctor["runtime"]["version"], env!("CARGO_PKG_VERSION"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn discovery_tool_visibility_matches_mcp_tools_list() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&["FIGHORSE_MCP_MODE"]);
    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "readonly") };

    let manifest = discovery::manifest();
    let visibility = &manifest["mcp"]["tool_visibility"];
    let readonly = visibility["readonly"]
        .as_array()
        .expect("readonly visibility tools");
    let write_mode = visibility["write_mode"]
        .as_array()
        .expect("write-mode visibility tools");

    let readonly_names = tool_names();
    for tool in readonly.iter().filter_map(|value| value.as_str()) {
        assert!(
            readonly_names.contains(tool),
            "readonly tools/list omits {tool}"
        );
    }
    for tool in write_mode.iter().filter_map(|value| value.as_str()) {
        assert!(
            !readonly_names.contains(tool),
            "readonly tools/list exposes write-only tool {tool}"
        );
    }
    assert!(readonly_names.contains("preview_code_connect"));
    assert!(readonly_names.contains("get_resource_catalog"));
    assert!(
        readonly
            .iter()
            .any(|value| value.as_str() == Some("get_resource_catalog"))
    );
    assert!(!readonly_names.contains("publish_code_connect"));

    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "write") };
    let write_names = tool_names();
    for tool in write_mode.iter().filter_map(|value| value.as_str()) {
        assert!(write_names.contains(tool), "write tools/list omits {tool}");
    }
    assert_eq!(
        visibility["code_connect_egress"]["env"],
        "FIGHORSE_MCP_CODE_CONNECT=allow"
    );
}

#[test]
fn quickstart_explains_figma_project_links_are_not_design_targets() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&["FIGHORSE_HOME", "FIGMA_TOKEN", "FIGMA_API_KEY"]);
    let root = temp_root("quickstart-project-link");
    unsafe { std::env::set_var("FIGHORSE_HOME", &root) };
    unsafe { std::env::remove_var("FIGMA_TOKEN") };
    unsafe { std::env::remove_var("FIGMA_API_KEY") };

    let report =
        discovery::quickstart(Some("https://www.figma.com/files/browser-root-placeholder"));
    let figma_check = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "figma_url")
        .unwrap();
    assert_eq!(figma_check["ok"], false);
    assert!(
        figma_check["message"]
            .as_str()
            .unwrap()
            .contains("cannot discover team IDs")
    );
    assert!(
        report["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|step| step.as_str())
            .any(|step| step.contains("Open a Figma team page"))
    );
    assert!(report["figma_url"]["file_key"].is_null());
    assert_eq!(report["figma_url"]["kind"], "files");
    assert_eq!(
        report["figma_url"]["browser_root_id"],
        "browser-root-placeholder"
    );
    assert!(report["figma_url"]["team_id"].is_null());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn quickstart_explains_team_browser_enumeration_permissions() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&["FIGHORSE_HOME", "FIGMA_TOKEN", "FIGMA_API_KEY"]);
    let root = temp_root("quickstart-team-link");
    unsafe { std::env::set_var("FIGHORSE_HOME", &root) };
    unsafe { std::env::remove_var("FIGMA_TOKEN") };
    unsafe { std::env::remove_var("FIGMA_API_KEY") };

    let report = discovery::quickstart(Some(
        "https://www.figma.com/files/browser-root-placeholder/team/team-id-placeholder",
    ));
    assert_eq!(report["figma_url"]["team_id"], "team-id-placeholder");
    let steps = report["next_steps"].as_array().unwrap();
    assert!(
        steps
            .iter()
            .filter_map(|step| step.as_str())
            .any(|step| step.contains("projects list <team-id>")),
        "{report}"
    );
    assert!(
        steps
            .iter()
            .filter_map(|step| step.as_str())
            .any(|step| step.contains("projects:read") && step.contains("403")),
        "{report}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn doctor_does_not_treat_an_active_lock_as_service_readiness() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&[
        "FIGHORSE_HOME",
        "FIGHORSE_MCP_LOCK_FILE",
        "FIGMA_TOKEN",
        "FIGMA_API_KEY",
    ]);
    let root = temp_root("doctor-readiness");
    let lock_path = root.join("isolated-mcp.lock");
    let port = {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.local_addr().unwrap().port()
    };
    fs::write(
        &lock_path,
        serde_json::to_vec(&json!({
            "kind":"fighorse.mcp-lock.v1",
            "pid":std::process::id(),
            "transport":"http",
            "port":port
        }))
        .unwrap(),
    )
    .unwrap();
    unsafe { std::env::set_var("FIGHORSE_HOME", &root) };
    unsafe { std::env::set_var("FIGHORSE_MCP_LOCK_FILE", &lock_path) };
    unsafe { std::env::remove_var("FIGMA_TOKEN") };
    unsafe { std::env::remove_var("FIGMA_API_KEY") };

    let report = discovery::doctor();
    assert_eq!(report["mcp_service"]["owner_active"], true);
    assert_eq!(report["mcp_service"]["listener_reachable"], false);
    assert_eq!(report["mcp_service"]["ready"], false);
    let service_check = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "mcp_service")
        .unwrap();
    assert_eq!(service_check["ok"], false);
    assert!(
        service_check["message"]
            .as_str()
            .unwrap()
            .contains("not ready")
    );

    let handshake = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "mcp_protocol")
        .unwrap();
    assert_eq!(handshake["ok"], false);
    assert!(
        !handshake["message"]
            .as_str()
            .unwrap()
            .contains("expected to")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn discovery_advertises_four_clients_and_streamable_http_without_legacy_sse() {
    let guidance = discovery::setup_guidance();
    let guidance_text = serde_json::to_string(&guidance).unwrap();
    for client in ["Cursor", "Codex", "Kimi", "Claude"] {
        assert!(
            guidance_text.contains(client),
            "missing {client}: {guidance}"
        );
    }
    assert!(guidance_text.contains("cursor,codex,kimi,claude"));

    let manifest = discovery::manifest();
    let manifest_text = serde_json::to_string(&manifest).unwrap();
    assert!(manifest_text.contains("Legacy SSE"));
    assert!(manifest_text.contains("--transport http"));
    assert!(!manifest_text.contains("\"/sse\""));
    assert!(!manifest_text.contains("\"/messages\""));
    assert!(!manifest_text.contains("fresh stateless transport"));
    assert!(!manifest_text.contains("persistent event store"));
}

#[test]
fn discovery_redacts_proxy_credentials() {
    let _lock = process_env_lock();
    let proxy_keys = [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ];
    let _env = EnvGuard::capture(&[
        "FIGHORSE_HOME",
        "FIGMA_TOKEN",
        "FIGMA_API_KEY",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ]);
    let root = temp_root("proxy-redaction");
    unsafe { std::env::set_var("FIGHORSE_HOME", &root) };
    unsafe { std::env::remove_var("FIGMA_TOKEN") };
    unsafe { std::env::remove_var("FIGMA_API_KEY") };
    for key in proxy_keys {
        unsafe { std::env::remove_var(key) };
    }
    unsafe {
        std::env::set_var(
            "HTTPS_PROXY",
            "http://proxy-user:proxy-secret@127.0.0.1:8080/path?token=hidden",
        )
    };

    for report in [discovery::doctor(), discovery::quickstart(None)] {
        let text = serde_json::to_string(&report).unwrap();
        assert!(report["proxy"]["configured"].as_bool().unwrap());
        assert!(!text.contains("proxy-user"), "{text}");
        assert!(!text.contains("proxy-secret"), "{text}");
        assert!(!text.contains("token=hidden"), "{text}");
    }
    let _ = fs::remove_dir_all(root);
}
