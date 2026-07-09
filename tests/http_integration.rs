//! HTTP client behavior tests using a mock server.
//!
//! Covers the HTTP client contract: verb selection,
//! auth header, JSON body, error surfacing, and empty-response handling. The
//! `FIGHORSE_API_BASE_URL` override points the real client at wiremock.
//!
//! These run under one `#[tokio::test]` so the shared process-global base-URL
//! env var is not mutated concurrently.

use fighorse::error::Error;
use fighorse::http;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test(flavor = "current_thread")]
async fn http_client_contract() {
    let server = MockServer::start().await;
    std::env::set_var("FIGHORSE_API_BASE_URL", server.uri());

    // POST: method, auth header, JSON body, JSON response.
    Mock::given(method("POST"))
        .and(path("/v1/post"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;
    let body = http::post("/v1/post", Some("token"), &[], Some(&json!({"a": 1})))
        .await
        .expect("post ok");
    assert_eq!(body, json!({"ok": true}));

    // PUT is supported.
    Mock::given(method("PUT"))
        .and(path("/v1/put"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;
    let body = http::put("/v1/put", Some("token"), &[], Some(&json!({"a": 1})))
        .await
        .expect("put ok");
    assert_eq!(body, json!({"ok": true}));

    // 204 empty body resolves to an empty object.
    Mock::given(method("DELETE"))
        .and(path("/v1/del"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let body = http::delete("/v1/del", Some("token"), &[]).await.expect("delete ok");
    assert_eq!(body, json!({}));

    // Non-2xx surfaces status + parsed body.
    Mock::given(method("GET"))
        .and(path("/v1/err"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({"message": "bad"})))
        .mount(&server)
        .await;
    match http::get("/v1/err", Some("token"), &[]).await {
        Err(Error::Figma { status, body, .. }) => {
            assert_eq!(status, 400);
            assert_eq!(body["message"], "bad");
        }
        other => panic!("expected Figma error, got {other:?}"),
    }

    // Verify the POST server-side view: method, header, exact body.
    let requests = server.received_requests().await.unwrap();
    let post_req = requests
        .iter()
        .find(|r| r.url.path() == "/v1/post")
        .expect("post request recorded");
    assert_eq!(post_req.method.as_str(), "POST");
    assert_eq!(post_req.headers.get("X-Figma-Token").unwrap(), "token");
    assert_eq!(std::str::from_utf8(&post_req.body).unwrap(), "{\"a\":1}");

    std::env::remove_var("FIGHORSE_API_BASE_URL");
}
