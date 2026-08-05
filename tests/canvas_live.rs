//! Opt-in live checks for the local Figma plugin bridge.
//!
//! These tests require the user to open dedicated blank Figma Design, FigJam,
//! and Slides files, run the fighorse Canvas Bridge plugin in each, and pair the
//! sessions with a running local bridge. They are ignored by default because
//! they depend on the Figma desktop app and manual editor state.

#[tokio::test]
#[ignore = "requires FIGHORSE_CANVAS_INTEGRATION_TESTS=1 and paired live Figma plugin sessions"]
async fn live_canvas_bridge_requires_manual_figma_sessions() {
    if std::env::var("FIGHORSE_CANVAS_INTEGRATION_TESTS").as_deref() != Ok("1") {
        eprintln!(
            "Set FIGHORSE_CANVAS_INTEGRATION_TESTS=1 after pairing blank Design, FigJam, and Slides sessions."
        );
        return;
    }

    let secret = fighorse::canvas::control::read_control_secret().unwrap();
    let port = fighorse::config::load_config().canvas_port;
    let response: serde_json::Value = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/canvas/sessions"))
        .header("x-fighorse-canvas-control", secret)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let sessions = response["sessions"].as_array().unwrap();
    let editors: std::collections::HashSet<_> = sessions
        .iter()
        .filter_map(|session| session["editor_type"].as_str())
        .collect();
    assert!(editors.contains("figma"));
    assert!(editors.contains("figjam"));
    assert!(editors.contains("slides"));
}
