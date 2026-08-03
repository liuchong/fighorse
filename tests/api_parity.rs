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
