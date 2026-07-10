//! MCP dispatch integration tests against a mock Figma API.
//!
//! Covers the MCP dispatch surface:
//! tool listing, policy gating, resource reads, and a Figma-backed tool call.

use fighorse::mcp::server::dispatch;
use serde_json::json;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "current_thread")]
async fn mcp_dispatch_end_to_end() {
    std::env::set_var("FIGHORSE_MCP_MODE", "readonly");
    std::env::set_var("FIGMA_TOKEN", "test-token");
    let server = MockServer::start().await;
    std::env::set_var("FIGHORSE_API_BASE_URL", server.uri());

    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/files/[^/]+$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "document": {"id": "0:0", "name": "Doc", "type": "DOCUMENT"}
        })))
        .mount(&server)
        .await;

    // initialize
    let init = dispatch(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}))
        .await
        .unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "fighorse");

    // tools/list — readonly excludes write tools.
    let list = dispatch(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await
        .unwrap();
    let names: Vec<String> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"get_file".to_string()));
    assert!(names.contains(&"figma_get_file".to_string()));
    assert!(!names.contains(&"post_comment".to_string()));

    // Policy: a write tool is rejected in readonly.
    let write_attempt = dispatch(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
        "params":{"name":"post_comment","arguments":{"file_key":"abc","message":"hi"}}}))
    .await
    .unwrap();
    assert_eq!(write_attempt["result"]["isError"], true);
    assert!(write_attempt["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("readonly mode"));

    // resources/read coverage.
    let cov = dispatch(&json!({"jsonrpc":"2.0","id":4,"method":"resources/read",
        "params":{"uri":"fighorse://coverage"}}))
    .await
    .unwrap();
    let text = cov["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("fighorse.api-coverage.v1"));

    // A Figma-backed tool call: get_file_tree hits the mock.
    let tree = dispatch(&json!({"jsonrpc":"2.0","id":5,"method":"tools/call",
        "params":{"name":"get_file_tree","arguments":{"file_key":"abc"}}}))
    .await
    .unwrap();
    let tree_text = tree["result"]["content"][0]["text"].as_str().unwrap();
    assert!(tree_text.contains("\"type\": \"DOCUMENT\""));

    // Unknown method → error with id.
    let bad = dispatch(&json!({"jsonrpc":"2.0","id":6,"method":"no/such"}))
        .await
        .unwrap();
    assert_eq!(bad["error"]["code"], -32601);

    std::env::remove_var("FIGHORSE_API_BASE_URL");
    std::env::remove_var("FIGMA_TOKEN");
    std::env::remove_var("FIGHORSE_MCP_MODE");
}
