use fighorse::mcp::tools;
use serde_json::json;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

fn process_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
fn canvas_mcp_tools_follow_write_and_script_gates() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&[
        "FIGHORSE_MCP_MODE",
        "FIGHORSE_CANVAS_MODE",
        "FIGHORSE_CANVAS_SCRIPT",
    ]);

    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "readonly") };
    unsafe { std::env::remove_var("FIGHORSE_CANVAS_MODE") };
    unsafe { std::env::remove_var("FIGHORSE_CANVAS_SCRIPT") };
    let readonly = tool_names();
    assert!(readonly.contains("canvas_status"));
    assert!(readonly.contains("canvas_create_pairing"));
    assert!(!readonly.contains("canvas_apply"));
    assert!(!readonly.contains("canvas_execute_script"));

    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "write") };
    unsafe { std::env::set_var("FIGHORSE_CANVAS_MODE", "readonly") };
    let mcp_write_only = tool_names();
    assert!(!mcp_write_only.contains("canvas_apply"));

    unsafe { std::env::set_var("FIGHORSE_CANVAS_MODE", "write") };
    let canvas_write = tool_names();
    assert!(canvas_write.contains("canvas_apply"));
    assert!(canvas_write.contains("canvas_upload_asset"));
    assert!(canvas_write.contains("canvas_undo"));
    assert!(!canvas_write.contains("canvas_execute_script"));

    unsafe { std::env::set_var("FIGHORSE_CANVAS_SCRIPT", "allow") };
    let script = tool_names();
    assert!(script.contains("canvas_execute_script"));
}

#[test]
fn direct_canvas_write_call_is_rejected_without_canvas_write_mode() {
    let _lock = process_env_lock();
    let _env = EnvGuard::capture(&["FIGHORSE_MCP_MODE", "FIGHORSE_CANVAS_MODE"]);
    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "write") };
    unsafe { std::env::set_var("FIGHORSE_CANVAS_MODE", "readonly") };

    let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
        tools::call_tool(
            "canvas_apply",
            &json!({
                "yes": true,
                "plan": {
                    "version": 1,
                    "session_id": "session-redacted",
                    "operations": []
                }
            }),
        )
        .await
    });

    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("FIGHORSE_CANVAS_MODE=write")
    );
}
