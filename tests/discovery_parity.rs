use fighorse::discovery;
use serde_json::json;
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
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
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
    std::env::set_var("FIGHORSE_HOME", &root);
    std::env::set_var("FIGHORSE_MCP_LOCK_FILE", &lock_path);
    std::env::remove_var("FIGMA_TOKEN");
    std::env::remove_var("FIGMA_API_KEY");

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
    std::env::set_var("FIGHORSE_HOME", &root);
    std::env::set_var("FIGHORSE_MCP_LOCK_FILE", &lock_path);
    std::env::remove_var("FIGMA_TOKEN");
    std::env::remove_var("FIGMA_API_KEY");

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
    assert!(service_check["message"]
        .as_str()
        .unwrap()
        .contains("not ready"));

    let handshake = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "mcp_protocol")
        .unwrap();
    assert_eq!(handshake["ok"], false);
    assert!(!handshake["message"]
        .as_str()
        .unwrap()
        .contains("expected to"));
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
    std::env::set_var("FIGHORSE_HOME", &root);
    std::env::remove_var("FIGMA_TOKEN");
    std::env::remove_var("FIGMA_API_KEY");
    for key in proxy_keys {
        std::env::remove_var(key);
    }
    std::env::set_var(
        "HTTPS_PROXY",
        "http://proxy-user:proxy-secret@127.0.0.1:8080/path?token=hidden",
    );

    for report in [discovery::doctor(), discovery::quickstart(None)] {
        let text = serde_json::to_string(&report).unwrap();
        assert!(report["proxy"]["configured"].as_bool().unwrap());
        assert!(!text.contains("proxy-user"), "{text}");
        assert!(!text.contains("proxy-secret"), "{text}");
        assert!(!text.contains("token=hidden"), "{text}");
    }
    let _ = fs::remove_dir_all(root);
}
