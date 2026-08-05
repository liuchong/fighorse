use std::fs;
use std::path::PathBuf;
use std::process::Command;
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

fn fighorse_json(args: &[&str], home: &PathBuf) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args(args)
        .env("FIGHORSE_HOME", home)
        .env_remove("FIGMA_TOKEN")
        .env_remove("FIGMA_API_KEY")
        .env_remove("FIGHORSE_CANVAS_MODE")
        .env_remove("FIGHORSE_CANVAS_SCRIPT")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn doctor_separates_rest_token_and_canvas_bridge_readiness() {
    let home = temp_root("smoke-cli-canvas");
    let report = fighorse_json(&["doctor", "--format", "json"], &home);

    assert_eq!(report["auth"]["has_token"], false);
    assert_eq!(report["auth"]["required_for_figma_api"], true);
    assert_eq!(report["canvas"]["requires_figma_rest_token"], false);
    assert_eq!(report["canvas"]["write_enabled"], false);
    assert_eq!(report["canvas"]["script_enabled"], false);
    assert!(
        report["canvas"]["session_readiness"]
            .as_str()
            .unwrap()
            .contains("canvas pair")
    );

    let _ = fs::remove_dir_all(home);
}
