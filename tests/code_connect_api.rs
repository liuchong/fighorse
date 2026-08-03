use fighorse::api::code_connect::{
    preview_documents, publish_documents, unpublish_documents, validate_documents,
};
use fighorse::code_connect::model::{CodeConnectDocument, TemplateData};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn doc(node_id: &str) -> CodeConnectDocument {
    CodeConnectDocument {
        figma_node: format!("https://www.figma.com/design/ABCDEF/Test?node-id={node_id}"),
        component: Some("Button".into()),
        variant: None,
        template:
            "const figma = require('figma'); export default { example: figma.code`<Button />` }"
                .into(),
        template_data: TemplateData {
            props: None,
            imports: None,
            nestable: Some(true),
            is_parserless: Some(true),
        },
        language: "jsx".into(),
        label: "React".into(),
        links: None,
        source: Some("src/Button.tsx".into()),
        source_location: Some(json!({"line": -1})),
        metadata: json!({"cliVersion": "1.5.1"}),
        code_connect_file_path: None,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn validates_previews_publishes_and_unpublishes_observed_protocol() {
    let server = MockServer::start().await;
    unsafe { std::env::set_var("FIGHORSE_API_BASE_URL", server.uri()) };

    Mock::given(method("GET"))
        .and(path("/v1/files/ABCDEF/nodes"))
        .and(header("X-Figma-Token", "token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "nodes": {
                "1:2": {
                    "document": {"type": "COMPONENT", "name": "Button", "componentPropertyDefinitions": {}},
                    "components": {"1:2": {}}
                }
            }
        })))
        .mount(&server)
        .await;

    let docs = vec![doc("1-2")];
    let validation = validate_documents("token", &docs).await.expect("validate");
    assert!(validation.valid);

    Mock::given(method("POST"))
        .and(path("/v1/code_connect/preview_snippets"))
        .and(query_param("file_key", "ABCDEF"))
        .and(header("X-Figma-Token", "token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {"results": [{"nodeId": "1:2", "success": true, "snippet": "<Button />", "language": "jsx"}]}
        })))
        .mount(&server)
        .await;

    let preview = preview_documents("token", &docs, None)
        .await
        .expect("preview");
    assert_eq!(preview["meta"]["results"][0]["snippet"], "<Button />");

    Mock::given(method("POST"))
        .and(path("/v1/code_connect"))
        .and(query_param("force", "true"))
        .and(header("X-Figma-Token", "token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "success": true,
                "published_count": 1,
                "failed_count": 0,
                "published_nodes": [{"figmaNode": docs[0].figma_node, "label": "React"}],
                "failed_nodes": []
            }
        })))
        .mount(&server)
        .await;

    let published = publish_documents("token", &docs, true, None)
        .await
        .expect("publish");
    assert_eq!(published["meta"]["published_count"], 1);

    Mock::given(method("DELETE"))
        .and(path("/v1/code_connect"))
        .and(header("X-Figma-Token", "token"))
        .and(body_json(json!({
            "nodes_to_delete": [{"figmaNode": docs[0].figma_node, "label": "React"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "success": true,
                "deleted_count": 1,
                "failed_count": 0,
                "deleted_nodes": [{"figmaNode": docs[0].figma_node, "label": "React"}],
                "failed_nodes": []
            }
        })))
        .mount(&server)
        .await;

    let deleted = unpublish_documents(
        "token",
        &[(
            "https://www.figma.com/design/ABCDEF/Test?node-id=1-2",
            "React",
        )],
    )
    .await
    .expect("unpublish");
    assert_eq!(deleted["meta"]["deleted_count"], 1);

    let incompatible_server = MockServer::start().await;
    unsafe { std::env::set_var("FIGHORSE_API_BASE_URL", incompatible_server.uri()) };

    Mock::given(method("POST"))
        .and(path("/v1/code_connect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"unexpected": true})))
        .mount(&incompatible_server)
        .await;

    let err = publish_documents("token", &[doc("1-2")], false, None)
        .await
        .expect_err("missing meta is incompatible");
    let msg = err.to_string();
    assert!(msg.contains("protocol_incompatible"), "{msg}");

    unsafe { std::env::remove_var("FIGHORSE_API_BASE_URL") };
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires FIGHORSE_CODE_CONNECT_E2E_DOCS and a real Figma token with Code Connect write access"]
async fn real_code_connect_preview_publish_unpublish_round_trip() {
    let token = std::env::var("FIGMA_TOKEN")
        .or_else(|_| std::env::var("FIGMA_ACCESS_TOKEN"))
        .expect("FIGMA_TOKEN or FIGMA_ACCESS_TOKEN required");
    let docs_path = std::env::var("FIGHORSE_CODE_CONNECT_E2E_DOCS")
        .expect("FIGHORSE_CODE_CONNECT_E2E_DOCS required");
    let docs_raw = std::fs::read_to_string(docs_path).expect("read docs fixture");
    let docs: Vec<CodeConnectDocument> = serde_json::from_str(&docs_raw).expect("parse docs");
    assert!(
        !docs.is_empty(),
        "docs fixture must contain at least one document"
    );

    let validation = validate_documents(&token, &docs)
        .await
        .expect("validate docs");
    assert!(validation.valid, "{validation:?}");
    let preview = preview_documents(&token, &docs, None)
        .await
        .expect("preview docs");
    assert!(preview.get("meta").is_some(), "{preview}");
    let publish = publish_documents(&token, &docs, true, None)
        .await
        .expect("publish docs");
    assert!(publish.get("meta").is_some(), "{publish}");
    let delete_nodes: Vec<(&str, &str)> = docs
        .iter()
        .map(|doc| (doc.figma_node.as_str(), doc.label.as_str()))
        .collect();
    let unpublish = unpublish_documents(&token, &delete_nodes)
        .await
        .expect("unpublish docs");
    assert!(unpublish.get("meta").is_some(), "{unpublish}");
}
