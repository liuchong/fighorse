use fighorse::api::{files, operations};
use fighorse::error::Error;
use fighorse::http;
use serde_json::json;
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn process_env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().await
}

struct BaseUrlGuard(Option<String>);

impl BaseUrlGuard {
    fn set(url: &str) -> Self {
        let previous = std::env::var("FIGHORSE_API_BASE_URL").ok();
        unsafe { std::env::set_var("FIGHORSE_API_BASE_URL", url) };
        Self(previous)
    }
}

impl Drop for BaseUrlGuard {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => unsafe { std::env::set_var("FIGHORSE_API_BASE_URL", value) },
            None => unsafe { std::env::remove_var("FIGHORSE_API_BASE_URL") },
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn file_paths_node_ids_and_queries_are_encoded_on_the_wire() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
        .mount(&server)
        .await;

    files::get_file_nodes(
        "test-token",
        "file/key?x=1",
        "1:2, 3:4&include=true",
        Some("version/a?b"),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let wire_url = requests[0].url.as_str();
    assert!(
        wire_url.contains("/v1/files/file%2Fkey%3Fx%3D1/nodes"),
        "{wire_url}"
    );
    assert!(
        wire_url.contains("ids=1%3A2%2C%203%3A4%26include%3Dtrue"),
        "{wire_url}"
    );
    assert!(wire_url.contains("version=version%2Fa%3Fb"), "{wire_url}");
}

#[tokio::test(flavor = "current_thread")]
async fn operation_dispatch_uses_encoded_path_templates_and_validates_required_params() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
        .mount(&server)
        .await;

    operations::call_operation(
        "test-token",
        "getCommentReactions",
        &json!({
            "file_key":"file/key",
            "comment_id":"comment/id?#",
            "cursor":"next page&all=true"
        }),
        &json!({}),
    )
    .await
    .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let wire_url = requests[0].url.as_str();
    assert!(
        wire_url.contains("/v1/files/file%2Fkey/comments/comment%2Fid%3F%23/reactions"),
        "{wire_url}"
    );
    assert!(
        wire_url.contains("cursor=next%20page%26all%3Dtrue"),
        "{wire_url}"
    );

    let missing = operations::call_operation("test-token", "getFile", &json!({}), &json!({}))
        .await
        .expect_err("missing path params must fail before sending a request");
    assert!(
        matches!(missing, Error::Usage(ref message) if message.contains("file_key")),
        "{missing}"
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "invalid operation params must not reach the network"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn non_json_api_errors_preserve_status_and_raw_body() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/raw-error"))
        .respond_with(ResponseTemplate::new(502).set_body_string("upstream exploded"))
        .mount(&server)
        .await;

    match http::get("/v1/raw-error", Some("test-token"), &[]).await {
        Err(Error::Figma { status, body, .. }) => {
            assert_eq!(status, 502);
            assert_eq!(body["raw"], "upstream exploded");
        }
        other => panic!("expected structured Figma error, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn auth_headers_distinguish_pat_plan_tokens_and_oauth_bearer_tokens() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());

    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "u1"})))
        .mount(&server)
        .await;

    unsafe { std::env::remove_var("FIGHORSE_AUTH_SCHEME") };
    http::get("/v1/me", Some("figd_pat_or_plan_token"), &[])
        .await
        .unwrap();
    http::get("/v1/me", Some("figu_oauth_access_token"), &[])
        .await
        .unwrap();

    unsafe { std::env::set_var("FIGHORSE_AUTH_SCHEME", "bearer") };
    http::get("/v1/me", Some("custom_oauth_token"), &[])
        .await
        .unwrap();
    unsafe { std::env::remove_var("FIGHORSE_AUTH_SCHEME") };

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);

    assert_eq!(
        requests[0]
            .headers
            .get("X-Figma-Token")
            .and_then(|v| v.to_str().ok()),
        Some("figd_pat_or_plan_token")
    );
    assert!(requests[0].headers.get("Authorization").is_none());

    assert_eq!(
        requests[1]
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok()),
        Some("Bearer figu_oauth_access_token")
    );
    assert!(requests[1].headers.get("X-Figma-Token").is_none());

    assert_eq!(
        requests[2]
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok()),
        Some("Bearer custom_oauth_token")
    );
    assert!(requests[2].headers.get("X-Figma-Token").is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn openapi_047_folders_teams_and_duplicate_operations_dispatch_cleanly() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/files/file-src/duplicate"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"file_key": "file-copy", "folder_id": "folder-1"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/teams/team-1/folders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"folders": []})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/teams/team-1/folders"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "folder-new", "name": "Mobile"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/folders/folder-1/folders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"folders": []})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/folders/folder-1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"files": []})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/folders/folder-1/meta"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "folder-1", "file_count": 3})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "team-1", "name": "Core"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/teams"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id": "team-new", "name": "Design Ops"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/teams/team-1/invites"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"invites": [{"email": "dev@example.com", "role": "editor"}], "errors": []}),
        ))
        .mount(&server)
        .await;

    let dup = operations::call_operation(
        "test-token",
        "postFileDuplicate",
        &json!({"file_key": "file-src"}),
        &json!({"folder_id": "folder-1"}),
    )
    .await
    .unwrap();
    assert_eq!(dup["file_key"], "file-copy");

    operations::call_operation(
        "test-token",
        "getTeamFolders",
        &json!({"team_id": "team-1"}),
        &json!({}),
    )
    .await
    .unwrap();

    let created_folder = operations::call_operation(
        "test-token",
        "postTeamFolder",
        &json!({"team_id": "team-1"}),
        &json!({"name": "Mobile", "sharing_audience_control": "plan_view"}),
    )
    .await
    .unwrap();
    assert_eq!(created_folder["id"], "folder-new");

    operations::call_operation(
        "test-token",
        "getFolderFolders",
        &json!({"folder_id": "folder-1"}),
        &json!({}),
    )
    .await
    .unwrap();
    operations::call_operation(
        "test-token",
        "getFolderFiles",
        &json!({"folder_id": "folder-1", "branch_data": true}),
        &json!({}),
    )
    .await
    .unwrap();
    let meta = operations::call_operation(
        "test-token",
        "getFolderMeta",
        &json!({"folder_id": "folder-1"}),
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(meta["file_count"], 3);

    let team = operations::call_operation(
        "test-token",
        "getTeam",
        &json!({"team_id": "team-1"}),
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(team["name"], "Core");

    let created_team = operations::call_operation(
        "test-token",
        "postTeam",
        &json!({}),
        &json!({"name": "Design Ops", "sharing_audience_control": "org_view"}),
    )
    .await
    .unwrap();
    assert_eq!(created_team["id"], "team-new");

    let invited = operations::call_operation(
        "test-token",
        "postTeamInvites",
        &json!({"team_id": "team-1"}),
        &json!({"emails": ["dev@example.com"], "role": "editor"}),
    )
    .await
    .unwrap();
    assert_eq!(invited["invites"][0]["role"], "editor");
}

#[tokio::test(flavor = "current_thread")]
async fn download_assets_and_search_design_system_cover_product_workflows() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());

    let render_url = format!("{}/cdn/render-1.png", server.uri());
    let fill_url = format!("{}/cdn/fill-hero.webp", server.uri());

    Mock::given(method("GET"))
        .and(path("/v1/images/file-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "images": {"1:2": render_url}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/files/file-1/nodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "nodes": {
                "1:2": {
                    "document": {
                        "id": "1:2",
                        "type": "FRAME",
                        "children": [
                            {
                                "id": "1:3",
                                "type": "RECTANGLE",
                                "fills": [{"type": "IMAGE", "imageRef": "img-hero-ref"}]
                            }
                        ]
                    }
                }
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/files/file-1/images"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {"images": {"img-hero-ref": fill_url}}
        })))
        .mount(&server)
        .await;

    let bundle = fighorse::export::images::download_assets(
        "test-token",
        "file-1",
        &["1:2".to_string()],
        &fighorse::export::images::DownloadAssetsOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(bundle["kind"], "fighorse.download_assets.v1");
    assert_eq!(bundle["export_renders"][0]["node_id"], "1:2");
    assert_eq!(bundle["raw_source_images"][0]["image_ref"], "img-hero-ref");
    assert_eq!(bundle["raw_images_truncated"], false);
    assert!(bundle["downloaded_to"].is_null());

    // Design system search across file components + local variables.
    Mock::given(method("GET"))
        .and(path("/v1/files/file-1/components"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "components": [
                    {"key": "comp-btn", "name": "PrimaryButton", "node_id": "10:1", "file_key": "file-1"}
                ]
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/files/file-1/component_sets"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"meta": {"component_sets": []}})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/files/file-1/styles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"meta": {"styles": []}})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/files/file-1/variables/local"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "variableCollections": {"col-1": {"name": "Semantic"}},
                "variables": {
                    "var-1": {
                        "key": "vkey-primary",
                        "name": "color/primary/button-bg",
                        "resolvedType": "COLOR",
                        "variableCollectionId": "col-1"
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let search = fighorse::product::library_search::search_design_system(
        "test-token",
        fighorse::product::library_search::DesignSystemSearchOpts {
            file_key: Some("file-1"),
            queries: vec!["button".to_string()],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(search["kind"], "fighorse.design-system-search.v1");
    assert_eq!(search["status"], "ready");
    assert_eq!(search["total_matches"], 2);
}
