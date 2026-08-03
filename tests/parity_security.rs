use fighorse::mcp::{server, tools};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, MutexGuard};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn process_env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().await
}

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("fighorse-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    root
}

struct ProcessState {
    cwd: PathBuf,
    env: Vec<(&'static str, Option<String>)>,
}

impl ProcessState {
    fn capture(keys: &[&'static str]) -> Self {
        Self {
            cwd: std::env::current_dir().unwrap(),
            env: keys
                .iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect(),
        }
    }
}

impl Drop for ProcessState {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.cwd);
        for (key, value) in &self.env {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

fn result_text(result: &serde_json::Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn singleton_lock_rejects_active_owner_cleans_stale_and_allows_override() {
    let _guard = process_env_lock().await;
    let _state = ProcessState::capture(&["FIGHORSE_MCP_LOCK_FILE", "FIGHORSE_MCP_ALLOW_MULTIPLE"]);
    let root = temp_root("singleton");
    let lock_path = root.join("isolated.lock");
    unsafe { std::env::set_var("FIGHORSE_MCP_LOCK_FILE", &lock_path) };
    unsafe { std::env::remove_var("FIGHORSE_MCP_ALLOW_MULTIPLE") };

    let lock = server::acquire_singleton_lock("http", 9449)
        .unwrap()
        .expect("singleton lock should be held");
    let error = server::acquire_singleton_lock("stdio", 9449)
        .err()
        .expect("active owner must reject another server");
    assert!(error.to_string().contains("already running"));
    drop(lock);
    assert!(!lock_path.exists(), "dropping the owner releases its lock");

    fs::write(&lock_path, r#"{"pid":999999999}"#).unwrap();
    let replacement = server::acquire_singleton_lock("stdio", 9449)
        .unwrap()
        .expect("stale lock should be replaced");
    let lock_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    assert_eq!(lock_json["pid"], std::process::id());
    drop(replacement);

    unsafe { std::env::set_var("FIGHORSE_MCP_ALLOW_MULTIPLE", "1") };
    assert!(
        server::acquire_singleton_lock("http", 9449)
            .unwrap()
            .is_none()
    );
    assert!(!lock_path.exists(), "override must not create a lock file");
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "current_thread")]
async fn mcp_export_policy_and_real_path_enforcement_are_independent() {
    let _guard = process_env_lock().await;
    let _state = ProcessState::capture(&[
        "FIGHORSE_API_BASE_URL",
        "FIGHORSE_MCP_MODE",
        "FIGHORSE_MCP_LOCAL_WRITE",
        "FIGHORSE_MCP_SERVICE",
        "FIGHORSE_HOME",
        "FIGMA_TOKEN",
    ]);
    let project = temp_root("export-policy");
    std::env::set_current_dir(&project).unwrap();
    unsafe { std::env::set_var("FIGMA_TOKEN", "test-token") };
    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "readonly") };
    unsafe { std::env::set_var("FIGHORSE_MCP_LOCAL_WRITE", "deny") };

    let denied = tools::call_tool(
        "export_images",
        &json!({"file_key":"file","node_ids":"1:2","dest_dir":"./.fighorse/exports"}),
    )
    .await;
    assert_eq!(denied["isError"], true);
    assert!(result_text(&denied).contains("FIGHORSE_MCP_LOCAL_WRITE=allow"));
    assert!(
        !result_text(&denied).contains("readonly mode"),
        "local export is not a Figma mutation"
    );

    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "write") };
    let still_denied = tools::call_tool(
        "export_images",
        &json!({"file_key":"file","node_ids":"1:2","dest_dir":"./.fighorse/exports"}),
    )
    .await;
    assert_eq!(still_denied["isError"], true);
    assert!(result_text(&still_denied).contains("FIGHORSE_MCP_LOCAL_WRITE=allow"));

    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "readonly") };
    unsafe { std::env::set_var("FIGHORSE_MCP_LOCAL_WRITE", "allow") };
    let mock = MockServer::start().await;
    unsafe { std::env::set_var("FIGHORSE_API_BASE_URL", mock.uri()) };
    Mock::given(method("GET"))
        .and(path("/v1/images/file"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"images": {}})))
        .mount(&mock)
        .await;

    let outside = project.join("outside");
    let escaped = tools::call_tool(
        "export_images",
        &json!({"file_key":"file","node_ids":"1:2","dest_dir":outside}),
    )
    .await;
    assert_eq!(escaped["isError"], true);
    assert!(result_text(&escaped).contains("outside allowed roots"));
    assert_eq!(
        mock.received_requests().await.unwrap().len(),
        0,
        "unsafe destinations must be rejected before the Figma API is called"
    );

    let traversal = project.join(".fighorse/exports/../../escaped");
    let traversed = tools::call_tool(
        "export_images",
        &json!({"file_key":"file","node_ids":"1:2","dest_dir":traversal}),
    )
    .await;
    assert_eq!(traversed["isError"], true);
    assert!(!project.join("escaped").exists());

    let dependency_path = project.join("node_modules/fighorse-assets");
    let dependency = tools::call_tool(
        "export_images",
        &json!({"file_key":"file","node_ids":"1:2","dest_dir":dependency_path}),
    )
    .await;
    assert_eq!(dependency["isError"], true);
    assert!(!project.join("node_modules/fighorse-assets").exists());

    #[cfg(unix)]
    {
        let system = PathBuf::from("/etc/fighorse-assets");
        let system_result = tools::call_tool(
            "export_images",
            &json!({"file_key":"file","node_ids":"1:2","dest_dir":system}),
        )
        .await;
        assert_eq!(system_result["isError"], true);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let external = temp_root("export-symlink-target");
        fs::create_dir_all(project.join(".fighorse")).unwrap();
        symlink(&external, project.join(".fighorse/exports")).unwrap();
        let symlinked = tools::call_tool(
            "export_images",
            &json!({"file_key":"file","node_ids":"1:2","dest_dir":"./.fighorse/exports"}),
        )
        .await;
        assert_eq!(symlinked["isError"], true);
        assert!(fs::read_dir(&external).unwrap().next().is_none());
        let _ = fs::remove_dir_all(external);
    }

    let _ = fs::remove_dir_all(project);
}

#[tokio::test(flavor = "current_thread")]
async fn approved_relative_and_absolute_export_paths_reach_the_real_writer() {
    let _guard = process_env_lock().await;
    let _state = ProcessState::capture(&[
        "FIGHORSE_API_BASE_URL",
        "FIGHORSE_MCP_MODE",
        "FIGHORSE_MCP_LOCAL_WRITE",
        "FIGHORSE_MCP_SERVICE",
        "FIGHORSE_HOME",
        "FIGMA_TOKEN",
    ]);
    let project = temp_root("export-success");
    std::env::set_current_dir(&project).unwrap();
    unsafe { std::env::set_var("FIGMA_TOKEN", "test-token") };
    unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "readonly") };
    unsafe { std::env::set_var("FIGHORSE_MCP_LOCAL_WRITE", "allow") };
    let mock = MockServer::start().await;
    unsafe { std::env::set_var("FIGHORSE_API_BASE_URL", mock.uri()) };
    let image_url = format!("{}/asset", mock.uri());

    Mock::given(method("GET"))
        .and(path("/v1/images/file"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"images":{"1:2":image_url,"3:4":image_url}})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/asset"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/png")
                .set_body_bytes(b"png".to_vec()),
        )
        .mount(&mock)
        .await;

    for destination in [
        PathBuf::from("./.fighorse/exports/relative"),
        project.join("assets/fighorse/absolute"),
    ] {
        let result = tools::call_tool(
            "export_images",
            &json!({
                "file_key":"file",
                "node_ids":"1:2,3:4",
                "dest_dir":destination,
                "manifest":true
            }),
        )
        .await;
        assert!(result.get("isError").is_none(), "{result}");
        let real = fs::canonicalize(&destination).unwrap();
        assert!(real.join("1_2.png").is_file());
        assert!(real.join("manifest.json").is_file());
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(real.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["kind"], "fighorse.image_export");
        assert!(
            manifest["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["node_id"] == "1:2")
        );
        assert!(real.starts_with(fs::canonicalize(&project).unwrap()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let destination = project.join(".fighorse/exports/leaf-symlink");
        fs::create_dir_all(&destination).unwrap();
        let outside_image = project.join("outside-image.txt");
        let outside_manifest = project.join("outside-manifest.txt");
        fs::write(&outside_image, "do-not-overwrite").unwrap();
        fs::write(&outside_manifest, "do-not-overwrite").unwrap();
        symlink(&outside_image, destination.join("1_2.png")).unwrap();
        symlink(&outside_manifest, destination.join("manifest.json")).unwrap();

        let result = tools::call_tool(
            "export_images",
            &json!({
                "file_key":"file",
                "node_ids":"1:2",
                "dest_dir":destination,
                "manifest":true
            }),
        )
        .await;
        assert!(result.get("isError").is_none(), "{result}");
        assert_eq!(
            fs::read_to_string(outside_image).unwrap(),
            "do-not-overwrite"
        );
        assert_eq!(
            fs::read_to_string(outside_manifest).unwrap(),
            "do-not-overwrite"
        );
        assert!(
            !fs::symlink_metadata(destination.join("1_2.png"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            !fs::symlink_metadata(destination.join("manifest.json"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    let service_home = project.join("service-home");
    unsafe { std::env::set_var("FIGHORSE_MCP_SERVICE", "1") };
    unsafe { std::env::set_var("FIGHORSE_HOME", &service_home) };
    let service_export = tools::call_tool(
        "export_images",
        &json!({"file_key":"file","node_ids":"1:2"}),
    )
    .await;
    assert!(service_export.get("isError").is_none(), "{service_export}");
    let exported: serde_json::Value = serde_json::from_str(result_text(&service_export)).unwrap();
    let path = PathBuf::from(exported["1:2"].as_str().unwrap());
    let stable_service_root = fs::canonicalize(service_home.join("exports")).unwrap();
    assert!(
        path.starts_with(&stable_service_root),
        "shared service exports must use a stable user root: {}",
        path.display()
    );

    let _ = fs::remove_dir_all(project);
}
