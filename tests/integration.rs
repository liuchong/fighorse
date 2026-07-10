//! Real Figma API integration tests.
//!
//! These tests hit the live Figma REST API using the configured token and a
//! real file key. They are `#[ignore]` by default and only run when both
//! `FIGMA_INTEGRATION_TESTS=1` and `FIGMA_TEST_FILE_KEY=<key>` are set, with a
//! valid token in fighorse config or `FIGMA_TOKEN`.
//!
//! Run serially to respect Figma's rate limit:
//!   FIGMA_INTEGRATION_TESTS=1 FIGMA_TEST_FILE_KEY=<key> \
//!     cargo test --test integration -- --ignored --test-threads=1
//!
//! No Figma data is ever written to committed files: assertions check structural
//! properties only, and any downloaded assets go to a temp dir cleaned up at the
//! end of each test. Tests are consolidated to minimize API calls (the file is
//! fetched once and reused across assertions).

#![cfg(test)]

use fighorse::api::files;
use fighorse::config;
use fighorse::experience::{self, Filters, ScopeOpts};
use fighorse::product::design_package::{get_design_package, PackageOpts};
use fighorse::transform::{compact, filter as tree_filter, tokens};
use serde_json::{json, Value};

/// Skip unless integration testing is explicitly enabled with a file key + token.
fn integration_setup() -> Option<(String, String)> {
    if std::env::var("FIGMA_INTEGRATION_TESTS").as_deref() != Ok("1") {
        return None;
    }
    let file_key = std::env::var("FIGMA_TEST_FILE_KEY").ok()?;
    let token = config::load_config()
        .token
        .filter(|t| !t.is_empty())
        .or_else(|| std::env::var("FIGMA_TOKEN").ok().filter(|t| !t.is_empty()))?;
    if file_key.is_empty() || token.is_empty() {
        return None;
    }
    Some((token, file_key))
}

macro_rules! require_integration {
    () => {
        match integration_setup() {
            Some(setup) => setup,
            None => {
                eprintln!("skipping (set FIGMA_INTEGRATION_TESTS=1 FIGMA_TEST_FILE_KEY=<key>)");
                return;
            }
        }
    };
}

/// Find the first FRAME/COMPONENT node id in a document tree (runtime discovery).
fn first_renderable_node(doc: &Value) -> Option<String> {
    fn walk(node: &Value) -> Option<String> {
        let ty = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if matches!(ty, "FRAME" | "COMPONENT" | "INSTANCE") {
            return node.get("id").and_then(|v| v.as_str()).map(String::from);
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                if let Some(id) = walk(child) {
                    return Some(id);
                }
            }
        }
        None
    }
    walk(doc)
}

/// One file fetch + meta, reused across file/transform assertions. ~2 API calls.
#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn file_and_transforms_integration() {
    let (token, file_key) = require_integration!();

    // 1) get_file_meta: structural fields present.
    let meta = files::get_file_meta(&token, &file_key).await.expect("meta");
    assert!(
        !meta["file"]["name"].as_str().unwrap_or("").is_empty(),
        "file has name"
    );

    // 2) get_file (single call): the document tree drives everything else.
    let file = files::get_file(
        &token,
        &file_key,
        files::GetFileParams {
            depth: Some("3"),
            ..Default::default()
        },
    )
    .await
    .expect("get_file");
    let doc = file.get("document").cloned().unwrap_or(Value::Null);
    assert_eq!(doc["type"], "DOCUMENT");

    // 3) compact: simplified tree has id/name/type.
    let compacted = compact::compact(&doc, Some(3), Some(4000));
    assert_eq!(compacted["type"], "DOCUMENT");
    assert!(compacted.get("id").is_some());

    // 4) tokens: extract from the simplified real tree.
    let simplified = compact::simplify_tree(&doc, Some(3));
    let extracted = tokens::extract_tokens(&simplified);
    assert!(!extracted.is_empty(), "tokens extracted from real file");
    let css = match tokens::format_tokens(&extracted, "css", "--figma-") {
        tokens::Formatted::Text(s) => s,
        _ => panic!("expected css text"),
    };
    assert!(css.contains(":root"));

    // 5) filter: text nodes exist in a real design file.
    let opts = tree_filter::FilterOpts {
        types: tree_filter::parse_types("TEXT"),
        ..Default::default()
    };
    let filtered = tree_filter::filter_tree(&doc, &opts);
    assert!(filtered.is_some(), "text nodes found in real file");

    // 6) discover a renderable node id for downstream tests (no extra API call).
    let node_id = first_renderable_node(&doc).expect("renderable node found");
    assert!(!node_id.is_empty());
}

/// Full design package pipeline. ~2 API calls (get_file + get_images).
#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn design_package_integration() {
    let (token, file_key) = require_integration!();

    let opts = PackageOpts {
        figma_url: Some(&file_key),
        file_key: Some(&file_key),
        node_id: None,
        depth: 2,
        max_tokens: 4000,
        include_screenshot: true,
        include_assets: false,
        screenshot_format: "png".to_string(),
        scale: 2.0,
        screenshot_limit: 4,
        platform: Some("web-react"),
        asset_format: Some("svg"),
    };
    let pkg = get_design_package(&token, opts)
        .await
        .expect("design package");

    assert_eq!(pkg["kind"], "fighorse.design-package.v1");
    assert_eq!(pkg["implementation_target"]["platform"], "web-react");
    assert_eq!(pkg["implementation_target"]["asset_format"], "svg");
    assert!(pkg.get("context").is_some(), "has context");
    assert!(pkg.get("tokens").is_some(), "has tokens");
    assert!(pkg.get("diagnostics").is_some(), "has diagnostics");
    assert!(
        pkg.get("learned_experience").is_some(),
        "has learned_experience"
    );
    assert!(pkg.get("next_tools").is_some(), "has next_tools");
}

/// MCP dispatch + real Figma call + image export. ~3 API calls.
#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn mcp_and_export_integration() {
    let (token, file_key) = require_integration!();
    std::env::set_var("FIGMA_TOKEN", &token);

    // 1) tools/list includes the new 0.41.0 operations.
    let list = fighorse::mcp::server::dispatch(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list"
    }))
    .await
    .unwrap();
    let names: Vec<String> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"figma_get_project_meta".to_string()));
    assert!(names.contains(&"figma_get_ai_usage_daily".to_string()));

    // 2) Real Figma call via MCP dispatch: get_file_meta (light endpoint).
    let result = fighorse::mcp::server::dispatch(&json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "get_file_meta", "arguments": {"file_key": file_key}}
    }))
    .await
    .unwrap();
    let text = result["result"]["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert!(!parsed["file"]["name"].as_str().unwrap_or("").is_empty());

    // 3) Image export to a temp dir under an approved root, then clean up.
    let file = files::get_file(
        &token,
        &file_key,
        files::GetFileParams {
            depth: Some("2"),
            ..Default::default()
        },
    )
    .await
    .expect("get_file for node discovery");
    let doc = file.get("document").cloned().unwrap_or(Value::Null);
    let node_id = first_renderable_node(&doc).expect("renderable node");

    let tmp = std::env::temp_dir().join(format!("fighorse-int-{}", std::process::id()));
    let export_dir = tmp.join("exports");
    std::fs::create_dir_all(&export_dir).unwrap();
    let export_dir_string = export_dir.to_string_lossy();

    let rows = fighorse::export::images::export_images(
        &token,
        &file_key,
        &[node_id],
        &fighorse::export::images::ExportOptions {
            format: "png",
            scale: "2",
            dest_dir: Some(&export_dir_string),
            manifest: true,
            prefix: None,
        },
    )
    .await
    .expect("export_images");

    assert!(!rows.is_empty(), "image exported");
    assert!(std::path::Path::new(&rows[0].1).exists(), "file downloaded");
    assert!(
        std::fs::metadata(&rows[0].1).unwrap().len() > 0,
        "non-empty file"
    );
    assert!(
        export_dir.join("manifest.json").exists(),
        "manifest written"
    );

    let _ = std::fs::remove_dir_all(&tmp);
    assert!(!tmp.exists(), "temp dir cleaned up");
    std::env::remove_var("FIGMA_TOKEN");
}

/// Local experience store round-trip (no Figma API calls).
#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn experience_store_integration() {
    let tmp = std::env::temp_dir().join(format!("fighorse-exp-int-{}", std::process::id()));
    std::env::set_var("FIGHORSE_EXPERIENCE_PATH", tmp.join("exp.jsonl"));

    let opts = ScopeOpts::default();
    let record = json!({
        "summary": "Integration test lesson",
        "lesson": "This record is created and read back during integration testing.",
        "category": "workflow",
        "platform": "web-react",
        "tags": "test,integration",
    });
    let write = experience::add(&record, &opts).expect("add");
    assert_eq!(write["kind"], "fighorse.experience-write.v1");

    let listed = experience::list_experiences(
        &Filters {
            platform: Some("web-react".into()),
            tag: Some("test".into()),
            ..Default::default()
        },
        10,
        &opts,
    );
    assert_eq!(listed["returned_count"], 1);

    let _ = std::fs::remove_dir_all(&tmp);
    std::env::remove_var("FIGHORSE_EXPERIENCE_PATH");
}
