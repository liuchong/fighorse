use futures_util::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::{json, Value};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Server {
    child: Child,
    base: String,
    home: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.home);
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.local_addr().unwrap().port()
}

async fn start_server() -> Server {
    start_server_with_policy("readonly", "deny", None).await
}

async fn start_server_with_policy(
    mode: &str,
    local_write: &str,
    lock_path: Option<&Path>,
) -> Server {
    let port = free_port();
    let home = temp_lock_path("http-home").parent().unwrap().to_path_buf();
    let mut command = Command::new(env!("CARGO_BIN_EXE_fighorse"));
    command
        .args([
            "mcp",
            "serve",
            "--transport",
            "http",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .env("FIGHORSE_MCP_MODE", mode)
        .env("FIGHORSE_MCP_LOCAL_WRITE", local_write)
        .env("FIGHORSE_HOME", &home)
        .env_remove("FIGMA_TOKEN")
        .env_remove("FIGMA_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    match lock_path {
        Some(path) => {
            command
                .env_remove("FIGHORSE_MCP_ALLOW_MULTIPLE")
                .env("FIGHORSE_MCP_LOCK_FILE", path);
        }
        None => {
            command.env("FIGHORSE_MCP_ALLOW_MULTIPLE", "1");
        }
    }
    let child = command.spawn().unwrap();
    let server = Server {
        child,
        base: format!("http://127.0.0.1:{port}"),
        home,
    };
    let client = reqwest::Client::new();
    for _ in 0..100 {
        if client
            .get(format!("{}/health", server.base))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
        {
            return server;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("MCP HTTP server did not become ready");
}

async fn mcp_post(
    client: &reqwest::Client,
    server: &Server,
    message: Value,
    session: Option<&str>,
) -> reqwest::Response {
    let mut request = client
        .post(format!("{}/mcp", server.base))
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .json(&message);
    if let Some(session) = session {
        request = request.header("mcp-session-id", session);
    }
    request.send().await.unwrap()
}

fn temp_lock_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("fighorse-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    root.join("mcp.lock")
}

async fn mcp_json(response: reqwest::Response) -> Value {
    let is_sse = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"));
    if !is_sse {
        return response.json().await.unwrap();
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(chunk) = stream.next().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk.unwrap()));
            while let Some(end) = buffer.find("\n\n") {
                let event = buffer[..end].to_string();
                buffer.drain(..end + 2);
                for line in event.lines() {
                    if let Some(data) = line.strip_prefix("data:") {
                        let data = data.trim();
                        if !data.is_empty() {
                            return serde_json::from_str(data).unwrap();
                        }
                    }
                }
            }
        }
        panic!("SSE response ended without a data event");
    })
    .await
    .expect("timed out waiting for SSE response")
}

#[tokio::test(flavor = "multi_thread")]
async fn streamable_http_supports_independent_repeated_handshakes() {
    let server = start_server().await;
    let client = reqwest::Client::new();

    for id in [1, 2] {
        let response = mcp_post(
            &client,
            &server,
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": format!("client-{id}"), "version": "1"}
                }
            }),
            None,
        )
        .await;
        assert!(response.status().is_success());
        let session = response
            .headers()
            .get("mcp-session-id")
            .and_then(|h| h.to_str().ok())
            .map(str::to_owned);
        let body = mcp_json(response).await;
        assert_eq!(body["result"]["serverInfo"]["name"], "fighorse");

        let initialized = mcp_post(
            &client,
            &server,
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            session.as_deref(),
        )
        .await;
        assert!(initialized.status().is_success());

        let listed = mcp_post(
            &client,
            &server,
            json!({"jsonrpc":"2.0","id":id + 10,"method":"tools/list","params":{}}),
            session.as_deref(),
        )
        .await;
        assert!(listed.status().is_success());
        let listed = mcp_json(listed).await;
        assert!(listed["result"]["tools"]
            .as_array()
            .is_some_and(|v| !v.is_empty()));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn official_http_handler_serves_resources_prompts_and_notifications() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let initialized = mcp_post(
        &client,
        &server,
        json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"initialize",
            "params":{
                "protocolVersion":"2025-06-18",
                "capabilities":{},
                "clientInfo":{"name":"official-handler-test","version":"1"}
            }
        }),
        None,
    )
    .await;
    assert!(initialized.status().is_success());
    assert!(initialized.headers().get(CONTENT_TYPE).is_some());
    let session = initialized
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let initialized_body = mcp_json(initialized).await;
    assert_eq!(initialized_body["result"]["protocolVersion"], "2025-06-18");
    assert!(initialized_body["result"]["capabilities"]["resources"].is_object());
    assert!(initialized_body["result"]["capabilities"]["prompts"].is_object());

    let notification = mcp_post(
        &client,
        &server,
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        session.as_deref(),
    )
    .await;
    assert!(notification.status().is_success());

    let resources = mcp_json(
        mcp_post(
            &client,
            &server,
            json!({"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}),
            session.as_deref(),
        )
        .await,
    )
    .await;
    assert!(resources["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|resource| resource["uri"] == "fighorse://coverage"));

    let resource = mcp_json(
        mcp_post(
            &client,
            &server,
            json!({"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"fighorse://coverage"}}),
            session.as_deref(),
        )
        .await,
    )
    .await;
    assert!(resource["result"]["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("fighorse.api-coverage.v1"));

    let prompts = mcp_json(
        mcp_post(
            &client,
            &server,
            json!({"jsonrpc":"2.0","id":4,"method":"prompts/list","params":{}}),
            session.as_deref(),
        )
        .await,
    )
    .await;
    assert!(prompts["result"]["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|prompt| prompt["name"] == "fighorse_design_replication"));

    let prompt = mcp_json(
        mcp_post(
            &client,
            &server,
            json!({
                "jsonrpc":"2.0",
                "id":5,
                "method":"prompts/get",
                "params":{
                    "name":"fighorse_design_replication",
                    "arguments":{
                        "figma_url":"https://www.figma.com/design/abc/Test?node-id=1-2",
                        "platform":"web-react",
                        "asset_format":"svg"
                    }
                }
            }),
            session.as_deref(),
        )
        .await,
    )
    .await;
    assert!(prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap()
        .contains("record reusable lessons"));
}

#[tokio::test(flavor = "multi_thread")]
async fn official_http_handler_keeps_figma_and_local_write_gates_independent() {
    for (mode, local_write, expected) in [
        ("readonly", "deny", "FIGHORSE_MCP_LOCAL_WRITE=allow"),
        ("write", "deny", "FIGHORSE_MCP_LOCAL_WRITE=allow"),
        ("readonly", "allow", "Figma Personal Access Token"),
    ] {
        let server = start_server_with_policy(mode, local_write, None).await;
        let client = reqwest::Client::new();
        let initialized = mcp_post(
            &client,
            &server,
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"initialize",
                "params":{
                    "protocolVersion":"2025-06-18",
                    "capabilities":{},
                    "clientInfo":{"name":"policy-test","version":"1"}
                }
            }),
            None,
        )
        .await;
        let session = initialized
            .headers()
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let _ = mcp_json(initialized).await;
        let result = mcp_json(
            mcp_post(
                &client,
                &server,
                json!({
                    "jsonrpc":"2.0",
                    "id":2,
                    "method":"tools/call",
                    "params":{
                        "name":"export_images",
                        "arguments":{
                            "file_key":"file",
                            "node_ids":"1:2",
                            "dest_dir":"./.fighorse/exports"
                        }
                    }
                }),
                session.as_deref(),
            )
            .await,
        )
        .await;
        assert_eq!(result["result"]["isError"], true, "{result}");
        assert!(
            result["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{result}"
        );
    }

    for (mode, expected) in [
        ("readonly", "readonly mode"),
        ("write", "Figma Personal Access Token"),
    ] {
        let server = start_server_with_policy(mode, "allow", None).await;
        let client = reqwest::Client::new();
        let initialized = mcp_post(
            &client,
            &server,
            json!({
                "jsonrpc":"2.0",
                "id":10,
                "method":"initialize",
                "params":{
                    "protocolVersion":"2025-06-18",
                    "capabilities":{},
                    "clientInfo":{"name":"official-write-policy-test","version":"1"}
                }
            }),
            None,
        )
        .await;
        let session = initialized
            .headers()
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let _ = mcp_json(initialized).await;
        let result = mcp_json(
            mcp_post(
                &client,
                &server,
                json!({
                    "jsonrpc":"2.0",
                    "id":11,
                    "method":"tools/call",
                    "params":{
                        "name":"figma_post_variables",
                        "arguments":{
                            "params":{"file_key":"file"},
                            "body":{"variables":[]}
                        }
                    }
                }),
                session.as_deref(),
            )
            .await,
        )
        .await;
        assert_eq!(result["result"]["isError"], true, "{result}");
        assert!(
            result["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{result}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn streamable_http_get_is_sse_and_manifest_stays_separate() {
    let server = start_server().await;
    let client = reqwest::Client::new();

    let initialized = mcp_post(
        &client,
        &server,
        json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"initialize",
            "params":{
                "protocolVersion":"2025-06-18",
                "capabilities":{},
                "clientInfo":{"name":"get-test","version":"1"}
            }
        }),
        None,
    )
    .await;
    let session = initialized
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let _ = mcp_json(initialized).await;

    let stream = client
        .get(format!("{}/mcp", server.base))
        .header(ACCEPT, "text/event-stream")
        .header("mcp-session-id", session)
        .send()
        .await
        .unwrap();
    assert_eq!(
        stream
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );

    let manifest: Value = client
        .get(format!("{}/manifest", server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(manifest.get("kind").is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn streamable_http_rejects_unapproved_origin() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/mcp", server.base))
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .header("origin", "https://evil.example")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"ping","params":{}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);

    for path in ["/", "/health", "/manifest", "/discover"] {
        let bad_origin = client
            .get(format!("{}{path}", server.base))
            .header("origin", "https://evil.example")
            .send()
            .await
            .unwrap();
        assert_eq!(
            bad_origin.status(),
            reqwest::StatusCode::FORBIDDEN,
            "{path} must enforce Origin"
        );

        let bad_host = client
            .get(format!("{}{path}", server.base))
            .header("host", "evil.example")
            .send()
            .await
            .unwrap();
        assert_eq!(
            bad_host.status(),
            reqwest::StatusCode::FORBIDDEN,
            "{path} must enforce Host"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn http_sigterm_drains_and_releases_the_singleton_lock() {
    let lock_path = temp_lock_path("http-lock-release");
    let mut server = start_server_with_policy("readonly", "deny", Some(&lock_path)).await;
    assert!(lock_path.is_file());

    #[cfg(unix)]
    unsafe {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        assert_eq!(kill(server.child.id() as i32, 15), 0);
    }
    #[cfg(not(unix))]
    server.child.kill().unwrap();

    let status = server.child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    for _ in 0..40 {
        if !lock_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        !lock_path.exists(),
        "graceful shutdown must release the lock"
    );
    let _ = fs::remove_dir_all(lock_path.parent().unwrap());
}

#[test]
fn legacy_sse_transport_fails_with_migration_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args(["mcp", "serve", "--transport", "sse"])
        .env("FIGHORSE_MCP_ALLOW_MULTIPLE", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--transport http"));
    assert!(stderr.contains("http://127.0.0.1:9449/mcp"));
}
