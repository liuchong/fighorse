use fighorse::install::clients::{ClientKind, ClientSpec};
use fighorse::install::model::{InstallPlan, InstallStep};
use fighorse::install::service::{
    activate_service, launchd_plist, rollback_service, systemd_unit, ServiceCommandRunner,
    ServiceState,
};
use fighorse::install::skills::{migrate_legacy, GeneratedSkillTemplates};
use fighorse::install::transaction::{
    load_manifest, rollback, verify_manifest, InstallTransaction, ManagedFile,
};
use fighorse::install::{install_auth, install_client, SUPPORTED_CLIENTS};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/install")
            .join(name),
    )
    .unwrap()
}

#[test]
fn service_and_cli_plans_have_canonical_order() {
    let service = InstallPlan::service(
        PathBuf::from("/tmp/fighorse-home"),
        "http://127.0.0.1:9449/mcp",
        vec![ClientKind::Cursor, ClientKind::Codex],
    );
    assert_eq!(
        service.steps,
        vec![
            InstallStep::Preflight,
            InstallStep::Backup,
            InstallStep::Binary,
            InstallStep::Service,
            InstallStep::HealthReady,
            InstallStep::Clients,
            InstallStep::Skills,
            InstallStep::Verified,
        ]
    );

    let cli = InstallPlan::cli(PathBuf::from("/tmp/fighorse-home"));
    assert_eq!(
        cli.steps,
        vec![
            InstallStep::Preflight,
            InstallStep::Backup,
            InstallStep::Binary,
            InstallStep::Skills,
            InstallStep::Verified,
        ]
    );
    assert!(!cli.steps.contains(&InstallStep::Service));
    assert!(!cli.steps.contains(&InstallStep::Clients));
}

#[test]
fn transaction_rolls_back_only_managed_files_in_reverse_order() {
    let root = temp_root("rollback");
    let managed = root.join("z-client.json");
    let created = root.join("a-new-rule.mdc");
    let unrelated = root.join("notes.txt");
    fs::write(&managed, "user-owned-before").unwrap();
    fs::write(&unrelated, "leave-me-alone").unwrap();

    let mut transaction = InstallTransaction::new(&root).unwrap();
    transaction
        .write_managed(&managed, b"managed-after")
        .unwrap();
    transaction
        .write_managed(&created, b"created-by-install")
        .unwrap();
    transaction.commit(None).unwrap();

    let report = rollback(&root).unwrap();
    assert!(report.rollback.iter().all(|item| item.ok));
    assert!(report.rollback[0].name.contains("a-new-rule.mdc"));
    assert!(report.rollback[1].name.contains("z-client.json"));
    assert_eq!(fs::read_to_string(&managed).unwrap(), "user-owned-before");
    assert!(!created.exists());
    assert_eq!(fs::read_to_string(&unrelated).unwrap(), "leave-me-alone");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn failed_transaction_restores_pending_writes_without_a_manifest() {
    let root = temp_root("pending-rollback");
    let original = root.join("original.txt");
    let created = root.join("created.txt");
    fs::write(&original, "before").unwrap();

    let mut transaction = InstallTransaction::new(&root).unwrap();
    transaction.write_managed(&original, b"after").unwrap();
    transaction.write_managed(&created, b"created").unwrap();
    let rollback = transaction.rollback_pending();

    assert!(rollback.iter().all(|check| check.ok));
    assert_eq!(fs::read_to_string(original).unwrap(), "before");
    assert!(!created.exists());
    assert!(!root.join("install/manifest.json").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn managed_writes_are_idempotent_and_manifest_is_token_free() {
    let root = temp_root("idempotent");
    let target = root.join("mcp.json");
    let payload = br#"{"mcpServers":{"fighorse":{"url":"http://127.0.0.1:9449/mcp"}}}"#;

    let mut first = InstallTransaction::new(&root).unwrap();
    first.write_managed(&target, payload).unwrap();
    first.commit(None).unwrap();
    let first_manifest = load_manifest(&root).unwrap();

    let mut second = InstallTransaction::new(&root).unwrap();
    second.write_managed(&target, payload).unwrap();
    second.commit(None).unwrap();
    let second_manifest = load_manifest(&root).unwrap();

    assert_eq!(
        first_manifest.managed_files[0].hash,
        second_manifest.managed_files[0].hash
    );
    assert_eq!(
        first_manifest.managed_files[0].backup,
        second_manifest.managed_files[0].backup
    );
    assert!(verify_manifest(&root).unwrap().iter().all(|check| check.ok));
    let serialized = serde_json::to_string(&second_manifest).unwrap();
    assert!(!serialized.to_lowercase().contains("token"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn client_config_verification_ignores_unrelated_mutation_but_detects_fighorse_drift() {
    let root = temp_root("client-semantic-verify");
    let target = root.join(".claude.json");
    let spec = ClientSpec::new(ClientKind::Claude, "http://127.0.0.1:9449/mcp");
    let installed = spec.merge_config(Some(r#"{"theme":"dark"}"#)).unwrap();

    let mut transaction = InstallTransaction::new(&root).unwrap();
    transaction
        .write_managed_client_config(&target, installed.as_bytes(), &spec)
        .unwrap();
    transaction.commit(None).unwrap();

    let mut client_owned: Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    client_owned["numStartups"] = json!(42);
    fs::write(&target, serde_json::to_vec_pretty(&client_owned).unwrap()).unwrap();
    let unrelated_checks = verify_manifest(&root).unwrap();
    assert!(
        unrelated_checks.iter().all(|check| check.ok),
        "{unrelated_checks:?}"
    );

    client_owned["mcpServers"]["fighorse"]["url"] = json!("http://127.0.0.1:9449/not-mcp");
    fs::write(&target, serde_json::to_vec_pretty(&client_owned).unwrap()).unwrap();
    let drift_checks = verify_manifest(&root).unwrap();
    assert!(
        drift_checks.iter().any(|check| !check.ok),
        "{drift_checks:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn known_legacy_skills_are_removed_verified_and_restored_by_rollback() {
    let root = temp_root("skill-known-migration");
    let install_home = root.join("fighorse-home");
    let user_home = root.join("user-home");
    let known_skill = fixture("legacy-known-skill.md");
    let templates = GeneratedSkillTemplates::new(&known_skill, "known agents", "known cursor rule");
    let candidates = [
        (
            user_home.join(".cursor/skills/fighorse/SKILL.md"),
            known_skill.clone(),
        ),
        (
            user_home.join(".codex/skills/fighorse/AGENTS.md"),
            "known agents".to_string(),
        ),
        (
            user_home.join(".kimi/skills/fighorse/cursor-rule.mdc"),
            "known cursor rule".to_string(),
        ),
        (
            user_home.join(".config/agents/skills/fighorse/SKILL.md"),
            known_skill.clone(),
        ),
        (
            user_home.join(".agents/skills/fighorse/AGENTS.md"),
            "known agents".to_string(),
        ),
        (
            user_home.join(".agents/skills/fighorse/cursor-rule.mdc"),
            "known cursor rule".to_string(),
        ),
        (
            user_home.join(".claude/skills/fighorse/AGENTS.md"),
            "known agents".to_string(),
        ),
        (
            user_home.join(".claude/skills/fighorse/cursor-rule.mdc"),
            "known cursor rule".to_string(),
        ),
    ];
    for (path, content) in &candidates {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    let mut transaction = InstallTransaction::new(&install_home).unwrap();
    let migration = migrate_legacy(&mut transaction, &user_home, &templates).unwrap();
    assert_eq!(migration.removed.len(), candidates.len());
    assert!(migration.conflicts.is_empty());
    assert_eq!(migration.backups.len(), candidates.len());
    assert!(migration.backups.iter().all(|path| path.is_file()));
    assert!(candidates.iter().all(|(path, _)| !path.exists()));
    transaction.commit(None).unwrap();

    let manifest = load_manifest(&install_home).unwrap();
    assert_eq!(
        manifest
            .managed_files
            .iter()
            .filter(|file| file.desired_absent)
            .count(),
        candidates.len()
    );
    assert!(verify_manifest(&install_home)
        .unwrap()
        .iter()
        .all(|check| check.ok));

    let mut repeated = InstallTransaction::new(&install_home).unwrap();
    let repeated_report = migrate_legacy(&mut repeated, &user_home, &templates).unwrap();
    repeated.commit(None).unwrap();
    assert!(repeated_report.removed.is_empty());
    assert!(repeated_report.backups.is_empty());
    assert!(repeated_report.conflicts.is_empty());

    let rollback = rollback(&install_home).unwrap();
    assert!(rollback.rollback.iter().all(|check| check.ok));
    for (path, content) in candidates {
        assert_eq!(fs::read_to_string(path).unwrap(), content);
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn customized_legacy_skill_is_preserved_backed_up_and_idempotent() {
    let root = temp_root("skill-conflict");
    let install_home = root.join("fighorse-home");
    let user_home = root.join("user-home");
    let custom = user_home.join(".cursor/skills/fighorse/SKILL.md");
    let custom_content = fixture("legacy-custom-skill.md");
    fs::create_dir_all(custom.parent().unwrap()).unwrap();
    fs::write(&custom, &custom_content).unwrap();
    let templates = GeneratedSkillTemplates::new("known skill", "known agents", "known rule");

    let mut first = InstallTransaction::new(&install_home).unwrap();
    let first_report = migrate_legacy(&mut first, &user_home, &templates).unwrap();
    first.commit(None).unwrap();
    assert!(first_report.removed.is_empty());
    assert_eq!(first_report.conflicts.len(), 1);
    assert_eq!(first_report.conflicts[0].path, custom);
    assert_eq!(first_report.backups.len(), 1);
    assert_eq!(
        fs::read_to_string(&first_report.backups[0]).unwrap(),
        custom_content
    );
    assert_eq!(fs::read_to_string(&custom).unwrap(), custom_content);

    let mut second = InstallTransaction::new(&install_home).unwrap();
    let second_report = migrate_legacy(&mut second, &user_home, &templates).unwrap();
    second.commit(None).unwrap();
    assert!(second_report.removed.is_empty());
    assert_eq!(second_report.conflicts.len(), 1);
    assert_eq!(second_report.backups, first_report.backups);
    assert_eq!(fs::read_to_string(&custom).unwrap(), custom_content);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_skill_apply_uses_canonical_targets_and_shared_migration() {
    let root = temp_root("install-skill-apply");
    let user_home = root.join("user-home");
    let install_home = root.join("fighorse-home");
    let review = root.join("review");
    let package = install_home.join("skills/fighorse");

    let generated = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "skill",
            "--dir",
            review.to_str().unwrap(),
            "--clients",
            "cursor,claude,codex,kimi",
        ])
        .env("HOME", &user_home)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let legacy = user_home.join(".codex/skills/fighorse/SKILL.md");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::copy(review.join("SKILL.md"), &legacy).unwrap();

    let applied = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "skill",
            "--apply",
            "--dir",
            package.to_str().unwrap(),
            "--home",
            install_home.to_str().unwrap(),
            "--clients",
            "cursor,claude,codex,kimi",
        ])
        .env("HOME", &user_home)
        .output()
        .unwrap();
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let output: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert!(output["migration"]["removed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|path| path.as_str() == Some(legacy.to_str().unwrap())));
    assert!(!legacy.exists());

    let canonical = [
        user_home.join(".agents/skills/fighorse/SKILL.md"),
        user_home.join(".claude/skills/fighorse/SKILL.md"),
        user_home.join(".cursor/rules/fighorse.mdc"),
    ];
    assert!(canonical.iter().all(|path| path.is_file()));
    assert_eq!(output["applied"].as_array().unwrap().len(), 3);
    assert!(!user_home.join(".cursor/skills/fighorse/SKILL.md").exists());
    assert!(!user_home.join(".kimi/skills/fighorse/SKILL.md").exists());
    assert!(!user_home
        .join(".config/agents/skills/fighorse/SKILL.md")
        .exists());
    assert!(load_manifest(&install_home)
        .unwrap()
        .managed_files
        .iter()
        .any(|file| file.path == legacy && file.desired_absent));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn old_manifest_entries_default_to_present_content() {
    let managed: ManagedFile = serde_json::from_value(json!({
        "path": "/tmp/legacy",
        "hash": "abc",
        "backup": null,
        "existed_before": true,
        "order": 1
    }))
    .unwrap();
    assert!(!managed.desired_absent);
}

#[test]
fn new_install_prunes_missing_present_entries_but_keeps_absence_contracts() {
    let root = temp_root("manifest-prune");
    let home = root.join("home");
    let present = root.join("present");
    let stale = root.join("stale");
    let retired = root.join("retired");

    let mut initial = InstallTransaction::new(&home).unwrap();
    initial.write_managed(&present, b"present").unwrap();
    initial.write_managed(&stale, b"stale").unwrap();
    initial.write_managed(&retired, b"retired").unwrap();
    initial.commit(None).unwrap();

    let mut remove = InstallTransaction::new(&home).unwrap();
    remove.remove_managed(&retired).unwrap();
    remove.commit(None).unwrap();
    fs::remove_file(&stale).unwrap();

    InstallTransaction::new(&home)
        .unwrap()
        .commit(None)
        .unwrap();
    let manifest = load_manifest(&home).unwrap();
    assert!(manifest
        .managed_files
        .iter()
        .any(|file| file.path == present));
    assert!(!manifest.managed_files.iter().any(|file| file.path == stale));
    assert!(manifest
        .managed_files
        .iter()
        .any(|file| file.path == retired && file.desired_absent));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn payload_and_service_renderers_match_fixtures() {
    let endpoint = "http://127.0.0.1:9449/mcp";
    let cases = [
        (ClientKind::Cursor, "cursor.json"),
        (ClientKind::Kimi, "kimi.json"),
        (ClientKind::Claude, "claude.json"),
    ];
    for (kind, name) in cases {
        let actual =
            serde_json::to_string_pretty(&ClientSpec::new(kind, endpoint).json_payload()).unwrap();
        assert_eq!(format!("{actual}\n"), fixture(name));
    }
    assert_eq!(
        ClientSpec::new(ClientKind::Codex, endpoint).toml_payload(),
        fixture("codex.toml")
    );
    assert_eq!(
        launchd_plist("/tmp/fighorse", 9449, "/tmp/home", false),
        fixture("launchd.plist")
    );
    assert_eq!(
        systemd_unit("/tmp/fighorse", 9449, "/tmp/home", false),
        fixture("systemd.service")
    );
}

#[test]
fn auth_merge_preserves_unknown_fields_and_never_returns_token() {
    let root = temp_root("auth");
    fs::write(
        root.join("config.json"),
        r#"{"future":{"enabled":true},"token":"old-secret"}"#,
    )
    .unwrap();
    let result = install_auth(Some("new-secret"), Some(root.to_str().unwrap()), true).unwrap();
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("config.json")).unwrap()).unwrap();

    assert_eq!(config["future"]["enabled"], true);
    assert_eq!(config["token"], "new-secret");
    let output = serde_json::to_string(&result).unwrap();
    assert!(!output.contains("new-secret"));
    assert!(!output.contains("old-secret"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.join("config.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn legacy_sse_client_install_is_rejected() {
    let root = temp_root("sse");
    for client in SUPPORTED_CLIENTS {
        let error = install_client(
            Some(client),
            Some(root.join(client).to_str().unwrap()),
            "sse",
            9449,
            "fighorse",
            None,
            false,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("--transport http"), "{client}: {error}");
        assert!(error.contains("/mcp"), "{client}: {error}");
    }
    let legacy = fighorse::discovery::mcp_config("generic", "sse", 9449, "fighorse")
        .unwrap_err()
        .to_string();
    assert!(legacy.contains("--transport http"));
    assert!(legacy.contains("/mcp"));
    let _ = fs::remove_dir_all(root);
}

#[derive(Default)]
struct FakeServiceRunner {
    results: VecDeque<Value>,
    calls: Vec<(String, Vec<String>)>,
}

impl ServiceCommandRunner for FakeServiceRunner {
    fn run(&mut self, command: &str, args: &[String]) -> fighorse::error::Result<Value> {
        self.calls.push((command.to_string(), args.to_vec()));
        Ok(self
            .results
            .pop_front()
            .unwrap_or_else(|| json!({"ok": true})))
    }
}

#[test]
fn failed_service_activation_unloads_a_fresh_service() {
    let mut runner = FakeServiceRunner {
        results: VecDeque::from([json!({"ok": true}), json!({"ok": false, "exit_code": 7})]),
        ..FakeServiceRunner::default()
    };
    let state = ServiceState::new("systemd", PathBuf::from("/tmp/fighorse-mcp.service"), false);

    let error = activate_service(&mut runner, &state)
        .unwrap_err()
        .to_string();
    assert!(error.contains("systemd_enable"));
    let rollback = rollback_service(&mut runner, &state);

    assert!(rollback.iter().all(|check| check.ok));
    assert!(runner
        .calls
        .iter()
        .any(|(_, args)| { args == &["--user", "disable", "fighorse-mcp.service",] }));
    assert!(runner
        .calls
        .iter()
        .any(|(_, args)| { args == &["--user", "stop", "fighorse-mcp.service",] }));
}

#[test]
fn production_failure_restores_auth_and_previous_service() {
    let root = temp_root("service-failure");
    let user_home = root.join("user");
    let home = root.join("fighorse-home");
    let target = home.join("bin").join("fighorse");
    let fake_bin = root.join("fake-bin");
    let log = root.join("service.log");
    let failed_once = root.join("failed-once");
    let service_file = user_home
        .join(".config")
        .join("systemd")
        .join("user")
        .join("fighorse-mcp.service");
    fs::create_dir_all(service_file.parent().unwrap()).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::write(
        home.join("config.json"),
        r#"{"token":"old-secret","future":{"enabled":true}}"#,
    )
    .unwrap();
    fs::write(&service_file, "old-service-definition\n").unwrap();

    let fake_systemctl = fake_bin.join("systemctl");
    fs::write(
        &fake_systemctl,
        r#"#!/bin/sh
echo "$*" >> "$FAKE_SERVICE_LOG"
case "$*" in
  *"show --property=LoadState --value"*)
    echo loaded
    ;;
  *"is-enabled"*)
    echo enabled
    ;;
  *"is-active"*)
    echo active
    ;;
  *"enable --now"*)
    if [ ! -e "$FAKE_SERVICE_FAILED_ONCE" ]; then
      : > "$FAKE_SERVICE_FAILED_ONCE"
      exit 7
    fi
    ;;
esac
exit 0
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&fake_systemctl).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_systemctl, permissions).unwrap();
    }

    let path = format!(
        "{}:{}",
        fake_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "self",
            "--apply",
            "--mode",
            "service",
            "--service",
            "systemd",
            "--clients",
            "none",
            "--home",
            home.to_str().unwrap(),
            "--source",
            env!("CARGO_BIN_EXE_fighorse"),
            "--target",
            target.to_str().unwrap(),
            "--link-dir",
            fake_bin.to_str().unwrap(),
            "--token",
            "new-secret",
        ])
        .env("HOME", &user_home)
        .env("PATH", path)
        .env("FAKE_SERVICE_LOG", &log)
        .env("FAKE_SERVICE_FAILED_ONCE", &failed_once)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("systemd_enable failed"), "{error}");
    assert!(error.contains("service_rollback_systemd_enable"), "{error}");
    assert!(!error.contains("new-secret"));

    let config: Value =
        serde_json::from_str(&fs::read_to_string(home.join("config.json")).unwrap()).unwrap();
    assert_eq!(config["token"], "old-secret");
    assert_eq!(config["future"]["enabled"], true);
    assert_eq!(
        fs::read_to_string(&service_file).unwrap(),
        "old-service-definition\n"
    );
    let calls = fs::read_to_string(log).unwrap();
    assert_eq!(calls.matches("enable --now").count(), 1);
    assert!(calls.contains("--user enable fighorse-mcp.service"));
    assert!(calls.contains("--user start fighorse-mcp.service"));
    assert_eq!(calls.matches("daemon-reload").count(), 2);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cli_only_apply_uses_temp_home_without_service_or_client_side_effects() {
    let root = temp_root("cli-only");
    let home = root.join(".fighorse");
    let user_home = root.join("user-home");
    let link_dir = root.join("path-bin");
    let target = home.join("bin").join("fighorse");
    fs::create_dir_all(&link_dir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "self",
            "--apply",
            "--mode",
            "cli",
            "--home",
            home.to_str().unwrap(),
            "--source",
            env!("CARGO_BIN_EXE_fighorse"),
            "--target",
            target.to_str().unwrap(),
            "--link-dir",
            link_dir.to_str().unwrap(),
        ])
        .env("HOME", &user_home)
        .env(
            "PATH",
            format!(
                "{}:{}",
                link_dir.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["report"]["plan"]["mode"], "cli");
    let completed = result["report"]["completed"].as_array().unwrap();
    assert!(!completed.contains(&serde_json::json!("service")));
    assert!(!completed.contains(&serde_json::json!("health_ready")));
    assert!(!completed.contains(&serde_json::json!("clients")));
    assert!(target.exists());
    assert!(home.join("install/manifest.json").exists());
    assert!(fs::read_dir(home.join("services"))
        .unwrap()
        .next()
        .is_none());
    assert!(fs::read_dir(home.join("clients")).unwrap().next().is_none());
    let _ = fs::remove_dir_all(root);
}
