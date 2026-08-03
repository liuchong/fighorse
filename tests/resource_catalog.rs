use fighorse::product::resource_catalog::{
    CatalogOpts, get_resource_catalog, local_catalog_outcome,
};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

struct MissingQueryParam(&'static str);

impl Match for MissingQueryParam {
    fn matches(&self, request: &Request) -> bool {
        !request.url.query_pairs().any(|(key, _)| key == self.0)
    }
}

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

fn fixture(name: &str) -> Value {
    let raw = match name {
        "team_projects" => include_str!("fixtures/resource_catalog/team_projects.json"),
        "project_a_files" => include_str!("fixtures/resource_catalog/project_a_files.json"),
        "project_b_files" => include_str!("fixtures/resource_catalog/project_b_files.json"),
        other => panic!("unknown fixture {other}"),
    };
    serde_json::from_str(raw).unwrap()
}

fn team_opts() -> CatalogOpts<'static> {
    CatalogOpts {
        figma_url: Some(
            "https://www.figma.com/files/browser-root-placeholder/team/team-id-placeholder",
        ),
        team_id: None,
        project_id: None,
        include_libraries: true,
        branch_data: true,
        probe_file_access: true,
        max_probes: 2,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn team_catalog_enumerates_projects_files_libraries_and_bounded_probes() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());

    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"user-placeholder"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("team_projects")))
        .mount(&server)
        .await;
    for (project, body) in [
        ("project-a", fixture("project_a_files")),
        ("project-b", fixture("project_b_files")),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/v1/projects/{project}/files")))
            .and(query_param("branch_data", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/components"))
        .and(query_param("page_size", "1000"))
        .and(MissingQueryParam("after"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "components": [{"key":"component-a","name":"Button"}],
                "cursor": {"after": 2}
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/components"))
        .and(query_param("page_size", "1000"))
        .and(query_param("after", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "components": [{"key":"component-b","name":"Card"}],
                "cursor": {}
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/component_sets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {"component_sets": [{"key":"set-a","name":"Controls"}], "cursor": {}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/styles"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {"styles": [{"key":"style-a","name":"Primary"}], "cursor": {}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/files/file-a"))
        .and(query_param("depth", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "document": {
                "type": "DOCUMENT",
                "children": [{"id":"0:1","name":"Page","type":"CANVAS"}]
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/files/file-b"))
        .and(query_param("depth", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "document": {
                "type": "DOCUMENT",
                "children": [{"id":"0:2","name":"Library","type":"CANVAS"}]
            }
        })))
        .mount(&server)
        .await;

    let outcome = get_resource_catalog("test-token", team_opts())
        .await
        .expect("catalog");
    let report = outcome.report;
    assert!(!outcome.blocked);
    assert_eq!(report["kind"], "fighorse.resource-catalog.v1");
    assert_eq!(report["status"], "ready");
    assert_eq!(report["auth_probe"]["ok"], true);
    assert_eq!(report["summary"]["projects"], 2);
    assert_eq!(report["summary"]["files"], 2);
    assert_eq!(report["summary"]["branches"], 1);
    assert_eq!(report["summary"]["components"], 2);
    assert_eq!(report["summary"]["component_sets"], 1);
    assert_eq!(report["summary"]["styles"], 1);
    assert_eq!(report["summary"]["probed_files"], 2);
    assert_eq!(report["summary"]["readable_files"], 2);
    assert_eq!(report["projects"][0]["files"][0]["access"]["page_count"], 1);
    assert_eq!(report["projects"][1]["files"][0]["access"]["page_count"], 1);
    assert!(
        report["next_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["tool"] == "get_design_package")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn browser_root_is_blocked_without_network_requests() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());
    let opts = CatalogOpts {
        figma_url: Some("https://www.figma.com/files/browser-root-placeholder"),
        include_libraries: false,
        probe_file_access: false,
        ..CatalogOpts::default()
    };
    let outcome = local_catalog_outcome(&opts)
        .expect("local source")
        .expect("blocked catalog report");

    assert!(outcome.blocked);
    assert_eq!(outcome.report["status"], "blocked");
    assert_eq!(
        outcome.report["diagnostics"][0]["code"],
        "browser_root_not_enumerable"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn project_file_list_failure_is_blocked_not_partial() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"user-placeholder"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/projects/project-placeholder/files"))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({"message":"Unavailable"})))
        .mount(&server)
        .await;

    let outcome = get_resource_catalog(
        "test-token",
        CatalogOpts {
            figma_url: Some("https://www.figma.com/project/project-placeholder/Project"),
            include_libraries: false,
            ..CatalogOpts::default()
        },
    )
    .await
    .expect("blocked project catalog");

    assert!(outcome.blocked);
    assert_eq!(outcome.report["status"], "blocked");
    assert_eq!(outcome.report["projects"][0]["status"], "failed");
}

#[tokio::test(flavor = "current_thread")]
async fn projects_429_preserves_unrequested_project_rows() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"user-placeholder"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("team_projects")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/projects/project-a/files"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({"message":"Rate limited"})))
        .mount(&server)
        .await;

    let outcome = get_resource_catalog(
        "test-token",
        CatalogOpts {
            include_libraries: false,
            probe_file_access: false,
            ..team_opts()
        },
    )
    .await
    .expect("rate-limited catalog");

    assert!(outcome.blocked);
    assert_eq!(outcome.report["projects"].as_array().unwrap().len(), 2);
    assert_eq!(outcome.report["projects"][0]["status"], "failed");
    assert_eq!(outcome.report["projects"][1]["status"], "not_attempted");
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|request| request.url.path().contains("project-b"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn valid_identity_and_projects_403_produce_actionable_blocked_report() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"user-placeholder"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/projects"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "message": "Insufficient scope for this endpoint",
            "secret_debug": "must-not-leak"
        })))
        .mount(&server)
        .await;

    let outcome = get_resource_catalog(
        "test-token",
        CatalogOpts {
            include_libraries: false,
            probe_file_access: false,
            ..team_opts()
        },
    )
    .await
    .expect("blocked catalog report");

    assert!(outcome.blocked);
    assert_eq!(outcome.report["auth_probe"]["ok"], true);
    assert_eq!(
        outcome.report["diagnostics"][0]["code"],
        "projects_access_forbidden"
    );
    assert_eq!(outcome.report["diagnostics"][0]["http_status"], 403);
    assert_eq!(
        outcome.report["diagnostics"][0]["message"],
        "Insufficient scope for this endpoint"
    );
    assert!(!outcome.report.to_string().contains("must-not-leak"));
    assert!(
        outcome.report["diagnostics"][0]["next_step"]
            .as_str()
            .unwrap()
            .contains("projects:read")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn one_project_failure_preserves_successful_files_as_partial() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"user-placeholder"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("team_projects")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/projects/project-a/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("project_a_files")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/projects/project-b/files"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({"err":"Project is not visible"})),
        )
        .mount(&server)
        .await;

    let outcome = get_resource_catalog(
        "test-token",
        CatalogOpts {
            include_libraries: false,
            probe_file_access: false,
            ..team_opts()
        },
    )
    .await
    .expect("partial catalog");

    assert!(!outcome.blocked);
    assert_eq!(outcome.report["status"], "partial");
    assert_eq!(outcome.report["summary"]["files"], 1);
    assert_eq!(outcome.report["projects"][0]["files"][0]["key"], "file-a");
    assert!(
        outcome.report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "project_files_forbidden")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn repeated_library_cursor_is_partial_without_duplicate_items() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    let _base = BaseUrlGuard::set(&server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"user-placeholder"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "projects": [{"id":"project-a","name":"Product"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/projects/project-a/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"files":[]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/components"))
        .and(MissingQueryParam("after"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "components": [{"key":"component-a","name":"Button"}],
                "cursor": {"after":"loop"}
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/teams/team-id-placeholder/components"))
        .and(query_param("after", "loop"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "meta": {
                "components": [{"key":"component-a","name":"Button"}],
                "cursor": {"after":"loop"}
            }
        })))
        .mount(&server)
        .await;
    for endpoint in ["component_sets", "styles"] {
        Mock::given(method("GET"))
            .and(path(format!("/v1/teams/team-id-placeholder/{endpoint}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "meta": {(endpoint): [], "cursor": {}}
            })))
            .mount(&server)
            .await;
    }

    let outcome = get_resource_catalog(
        "test-token",
        CatalogOpts {
            probe_file_access: false,
            ..team_opts()
        },
    )
    .await
    .expect("partial catalog");

    assert_eq!(outcome.report["status"], "partial");
    assert_eq!(outcome.report["summary"]["components"], 1);
    assert_eq!(
        outcome.report["team_library"]["components"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test(flavor = "current_thread")]
async fn cli_resource_catalog_prints_the_shared_report() {
    let _lock = process_env_lock().await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"user-placeholder"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/projects/project-placeholder/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "Project",
            "files": [{"key":"file-placeholder","name":"File","branches":[]}]
        })))
        .mount(&server)
        .await;

    let base_url = server.uri();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(env!("CARGO_BIN_EXE_fighorse"))
            .args([
                "resource",
                "catalog",
                "https://www.figma.com/project/project-placeholder/Project",
                "--no-libraries",
            ])
            .env("FIGMA_TOKEN", "test-token")
            .env("FIGHORSE_API_BASE_URL", base_url)
            .output()
    })
    .await
    .expect("join CLI")
    .expect("run CLI");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("CLI JSON");
    assert_eq!(report["kind"], "fighorse.resource-catalog.v1");
    assert_eq!(report["status"], "ready");
    assert_eq!(report["summary"]["files"], 1);
}
