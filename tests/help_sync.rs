use std::process::Command;

fn fighorse(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn canvas_help_is_shared_between_help_routes() {
    let direct = fighorse(&["canvas", "--help"]);
    let detailed = fighorse(&["help", "canvas"]);

    assert_eq!(direct, detailed);
    for token in [
        "canvas serve",
        "canvas apply",
        "canvas_execute_script",
        "FIGHORSE_CANVAS_MODE",
        "FIGHORSE_CANVAS_SCRIPT",
    ] {
        assert!(direct.contains(token), "canvas help omits {token}");
    }
}

#[test]
fn root_help_lists_canvas_bridge_entry_points() {
    let help = fighorse(&["--help"]);
    for token in [
        "canvas status|pair|sessions",
        "canvas upload-asset",
        "FIGHORSE_CANVAS_BRIDGE",
        "install ai-plugin",
        "workflow skills",
    ] {
        assert!(help.contains(token), "root help omits {token}");
    }
}
