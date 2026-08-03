//! Design package assembly test against a mock Figma API.
//!
//! Builds a self-contained
//! replication package from mocked file-nodes + images responses.

use fighorse::product::design_package::{PackageOpts, get_design_package};
use serde_json::json;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "current_thread")]
async fn design_package_from_figma_url() {
    let server = MockServer::start().await;
    unsafe { std::env::set_var("FIGHORSE_API_BASE_URL", server.uri()) };
    // Isolate the experience store so the package's learned_experience is empty.
    let exp = std::env::temp_dir().join(format!("fh-dp-{}.jsonl", std::process::id()));
    unsafe { std::env::set_var("FIGHORSE_EXPERIENCE_PATH", &exp) };

    let mock_node = json!({
        "id": "1:2",
        "name": "Hero Card",
        "type": "FRAME",
        "absoluteBoundingBox": {"width": 320, "height": 180},
        "fills": [{"type": "SOLID", "color": {"r": 0.2, "g": 0.4, "b": 0.8, "a": 1}}],
        "children": [{
            "id": "1:3", "name": "Title", "type": "TEXT", "characters": "Hello",
            "style": {"fontFamily": "Test Sans", "fontSize": 24, "fontWeight": 700},
            "absoluteBoundingBox": {"width": 120, "height": 32}
        }]
    });

    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/images/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "images": {"1:2": "https://images.example/hero.png"}
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/files/.*/nodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "Mock File",
            "lastModified": "2026-05-15T00:00:00Z",
            "nodes": {"1:2": {"document": mock_node}}
        })))
        .mount(&server)
        .await;

    let opts = PackageOpts {
        figma_url: Some("https://www.figma.com/design/abc/Mock?node-id=1-2"),
        file_key: None,
        node_id: None,
        depth: 2,
        max_tokens: 2000,
        include_screenshot: true,
        include_assets: false,
        screenshot_format: "png".to_string(),
        scale: 2.0,
        screenshot_limit: 4,
        platform: Some("android-compose"),
        asset_format: Some("png"),
    };
    let pkg = get_design_package("token", opts).await.expect("package");

    assert_eq!(pkg["kind"], "fighorse.design-package.v1");
    assert_eq!(pkg["source"]["file_key"], "abc");
    assert_eq!(pkg["source"]["node_id"], "1:2");
    assert_eq!(pkg["implementation_target"]["platform"], "android-compose");
    assert_eq!(pkg["implementation_target"]["asset_format"], "png");
    assert!(
        pkg["implementation_target"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_str().unwrap_or("").contains("Jetpack Compose"))
    );
    assert!(
        !pkg["fidelity_workflow"]["attention_checks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        pkg["asset_export_plan"]["mcp_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["tool"] == "export_component")
    );
    assert_eq!(pkg["ai_contract"]["kind"], "fighorse.ai-contract.v1");
    assert!(
        pkg["asset_export_plan"]["cli_examples"][0]
            .as_str()
            .unwrap()
            .contains(".fighorse/exports")
    );
    assert_eq!(
        pkg["learned_experience"]["kind"],
        "fighorse.learned-guidance.v1"
    );
    assert!(
        pkg["next_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["tool"] == "record_experience")
    );
    assert_eq!(pkg["target"]["name"], "Hero Card");
    assert_eq!(
        pkg["screenshots"]["images"]["1:2"],
        "https://images.example/hero.png"
    );
    assert!(!pkg["tokens"]["color"].as_array().unwrap().is_empty());
    assert_eq!(pkg["diagnostics"]["status"], "ready");
    assert_eq!(pkg["diagnostics"]["screenshots"]["count"], 1);
    assert_eq!(pkg["diagnostics"]["context_truncated"], false);

    unsafe { std::env::remove_var("FIGHORSE_API_BASE_URL") };
    unsafe { std::env::remove_var("FIGHORSE_EXPERIENCE_PATH") };
    let _ = std::fs::remove_file(&exp);
}
