use fighorse::install::service::systemd_unit;
use fighorse::install::transaction::{InstallTransaction, load_manifest};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
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

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(unix)]
fn write_executable(path: &std::path::Path, content: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn managed_files_preserve_modes_symlink_type_and_secure_installer_storage() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = temp_root("metadata");
    let install_home = root.join("home");
    let regular = root.join("regular.txt");
    let link_target = root.join("link-target.txt");
    let link = root.join("managed-link");
    fs::write(&regular, "regular-before").unwrap();
    fs::write(&link_target, "target-before").unwrap();
    fs::set_permissions(&regular, fs::Permissions::from_mode(0o640)).unwrap();
    fs::set_permissions(&link_target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&link_target, &link).unwrap();

    let mut transaction = InstallTransaction::new(&install_home).unwrap();
    transaction
        .write_managed(&regular, b"regular-after")
        .unwrap();
    let symlink_write = transaction.write_managed(&link, b"link-after");
    assert!(symlink_write.is_err());
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(&link).unwrap(), link_target);
    transaction.remove_managed(&link).unwrap();
    let pending = transaction.rollback_pending();
    assert!(pending.iter().all(|check| check.ok), "{pending:?}");

    assert_eq!(fs::read_to_string(&regular).unwrap(), "regular-before");
    assert_eq!(mode(&regular), 0o640);
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(&link).unwrap(), link_target);
    assert_eq!(fs::read_to_string(&link_target).unwrap(), "target-before");

    let mut committed = InstallTransaction::new(&install_home).unwrap();
    committed.write_managed(&regular, b"committed").unwrap();
    committed.commit(None).unwrap();
    let manifest = load_manifest(&install_home).unwrap();
    let backup = manifest.managed_files[0].backup.as_ref().unwrap();
    assert_eq!(mode(&install_home.join("install")), 0o700);
    assert_eq!(mode(&install_home.join("install/backups")), 0o700);
    assert_eq!(mode(backup), 0o600);
    assert_eq!(mode(&install_home.join("install/manifest.json")), 0o600);
    assert!(
        fs::read_dir(install_home.join("install"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp-"))
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn manual_rollback_restores_persisted_service_state() {
    let root = temp_root("manual-service-rollback");
    let home = root.join("fighorse-home");
    let user_home = root.join("user-home");
    let fake_bin = root.join("bin");
    let log = root.join("systemctl.log");
    fs::create_dir_all(home.join("install")).unwrap();
    fs::create_dir_all(&fake_bin).unwrap();
    fs::write(
        home.join("install/manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 3,
            "managed_files": [],
            "last_verification": null,
            "endpoint": "http://127.0.0.1:9555/mcp",
            "service": {
                "manager": "systemd",
                "target": user_home.join(".config/systemd/user/fighorse-mcp.service"),
                "before": {"loaded": true, "enabled": false, "running": false}
            }
        }))
        .unwrap(),
    )
    .unwrap();
    write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\necho \"$*\" >> \"$SERVICE_LOG\"\nexit 0\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args(["install", "rollback", "--home", home.to_str().unwrap()])
        .env("HOME", &user_home)
        .env("SERVICE_LOG", &log)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
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
    let calls = fs::read_to_string(&log).unwrap();
    assert!(calls.contains("daemon-reload"), "{calls}");
    assert!(calls.contains("disable"), "{calls}");
    assert!(calls.contains("stop"), "{calls}");
    assert!(!calls.contains("enable --now"), "{calls}");
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn install_service_apply_fails_checked_activation_and_rolls_back() {
    let root = temp_root("service-entry");
    let home = root.join("fighorse-home");
    let user_home = root.join("user-home");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    write_executable(
        &fake_bin.join("systemctl"),
        r#"#!/bin/sh
case "$*" in
  *"is-enabled"*|*"is-active"*) exit 1 ;;
  *"enable --now"*) exit 7 ;;
esac
exit 0
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "service",
            "--apply",
            "--service",
            "systemd",
            "--home",
            home.to_str().unwrap(),
            "--command",
            env!("CARGO_BIN_EXE_fighorse"),
        ])
        .env("HOME", &user_home)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!home.join("services/fighorse-mcp.service").exists());
    assert!(
        !user_home
            .join(".config/systemd/user/fighorse-mcp.service")
            .exists()
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn cli_only_install_manages_path_link_and_shared_skill_without_mcp_side_effects() {
    let root = temp_root("cli-links");
    let home = root.join("fighorse-home");
    let user_home = root.join("user-home");
    let link_dir = root.join("path-bin");
    let target = home.join("bin/fighorse");
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
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["report"]["ok"], true, "{result}");
    assert!(link_dir.join("fighorse").exists());
    assert!(user_home.join(".agents/skills/fighorse/SKILL.md").is_file());
    assert!(
        fs::read_dir(home.join("services"))
            .unwrap()
            .next()
            .is_none()
    );
    assert!(fs::read_dir(home.join("clients")).unwrap().next().is_none());
    let manifest = load_manifest(&home).unwrap();
    assert!(
        manifest
            .managed_files
            .iter()
            .any(|file| file.path == link_dir.join("fighorse"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_install_merges_user_content_and_failure_restores_everything() {
    let root = temp_root("project-transaction");
    let home = root.join("fighorse-home");
    let project = root.join("project");
    let scoped = project.join(".fighorse");
    fs::create_dir_all(&scoped).unwrap();
    let config = scoped.join("fighorse.json");
    let ignore = scoped.join(".gitignore");
    let readme = scoped.join("README.md");
    fs::write(&config, r#"{"future":{"keep":true}}"#).unwrap();
    fs::write(&ignore, "user-ignore\n").unwrap();
    fs::write(&readme, "# User README\n\nkeep this\n").unwrap();

    let failed = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "all",
            "--apply",
            "--mode",
            "cli",
            "--home",
            home.to_str().unwrap(),
            "--path",
            project.to_str().unwrap(),
            "--source",
            root.join("missing-binary").to_str().unwrap(),
            "--link-dirs",
            "none",
        ])
        .env("HOME", root.join("user-home"))
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        r#"{"future":{"keep":true}}"#
    );
    assert_eq!(fs::read_to_string(&ignore).unwrap(), "user-ignore\n");
    assert_eq!(
        fs::read_to_string(&readme).unwrap(),
        "# User README\n\nkeep this\n"
    );

    let merged = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "project",
            "--project-dir",
            project.to_str().unwrap(),
        ])
        .env("FIGHORSE_HOME", &home)
        .output()
        .unwrap();
    assert!(merged.status.success());
    let merged_config: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(merged_config["future"]["keep"], true);
    assert!(
        fs::read_to_string(&ignore)
            .unwrap()
            .contains("user-ignore\n")
    );
    assert!(
        fs::read_to_string(&readme)
            .unwrap()
            .contains("# User README")
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn auth_login_and_logout_preserve_unknown_fields_and_secure_mode() {
    let root = temp_root("auth-cli");
    let home = root.join("fighorse-home");
    fs::create_dir_all(&home).unwrap();
    let config = home.join("config.json");
    fs::write(&config, r#"{"future":{"keep":true},"token":"old"}"#).unwrap();
    let login = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args(["auth", "login", "--token", "new-secret"])
        .env("FIGHORSE_HOME", &home)
        .output()
        .unwrap();
    assert!(login.status.success());
    let after_login: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(after_login["future"]["keep"], true);
    assert_eq!(after_login["token"], "new-secret");
    assert_eq!(mode(&config), 0o600);

    let logout = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args(["auth", "logout"])
        .env("FIGHORSE_HOME", &home)
        .output()
        .unwrap();
    assert!(logout.status.success());
    let after_logout: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(after_logout["future"]["keep"], true);
    assert!(after_logout.get("token").is_none());
    assert_eq!(mode(&config), 0o600);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn systemd_renderer_quotes_all_dynamic_values() {
    let actual = systemd_unit(
        "/tmp/Fig Horse\\bin\"ary%q",
        9449,
        "/tmp/Home Dir\\x\"y%z",
        false,
    );
    let expected = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/install/systemd-escaped.service"),
    )
    .unwrap();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn verify_uses_manifest_endpoint_unless_port_is_explicit() {
    let root = temp_root("verify-endpoint");
    let home = root.join("home");
    let service_file = root.join("fighorse-mcp.service");
    let content = b"service";
    fs::write(&service_file, content).unwrap();
    fs::create_dir_all(home.join("install")).unwrap();
    let hash: String = Sha256::digest(content)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    fs::write(
        home.join("install/manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 3,
            "managed_files": [{
                "path": service_file,
                "hash": hash,
                "backup": null,
                "existed_before": true,
                "desired_absent": false,
                "order": 1
            }],
            "last_verification": null,
            "endpoint": "http://127.0.0.1:65534/mcp",
            "service": null
        }))
        .unwrap(),
    )
    .unwrap();

    let default = fighorse::install::install_verify(Some(home.to_str().unwrap()), 0)
        .await
        .unwrap();
    let default_text = serde_json::to_string(&default).unwrap();
    assert!(default_text.contains("65534"), "{default_text}");

    let explicit = fighorse::install::install_verify(Some(home.to_str().unwrap()), 65533)
        .await
        .unwrap();
    let explicit_text = serde_json::to_string(&explicit).unwrap();
    assert!(explicit_text.contains("65533"), "{explicit_text}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn historical_generated_templates_are_removed_and_missing_managed_legacy_becomes_absent() {
    let root = temp_root("historical-skills");
    let install_home = root.join("fighorse-home");
    let user_home = root.join("user-home");
    let cursor_legacy = user_home.join(".cursor/skills/fighorse/SKILL.md");
    let codex_legacy = user_home.join(".codex/skills/fighorse/AGENTS.md");
    let cursor_rule = user_home.join(".agents/skills/fighorse/cursor-rule.mdc");
    for path in [&cursor_legacy, &codex_legacy, &cursor_rule] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    fs::write(
        &cursor_legacy,
        include_str!("fixtures/install/edb26d2-skill.md"),
    )
    .unwrap();
    fs::write(
        &codex_legacy,
        include_str!("fixtures/install/edb26d2-agents.md"),
    )
    .unwrap();
    fs::write(
        &cursor_rule,
        include_str!("fixtures/install/edb26d2-cursor-rule.mdc"),
    )
    .unwrap();

    let missing_legacy = user_home.join(".kimi/skills/fighorse/SKILL.md");
    fs::create_dir_all(install_home.join("install")).unwrap();
    fs::write(
        install_home.join("install/manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 2,
            "managed_files": [{
                "path": missing_legacy,
                "hash": "old-generated-hash",
                "backup": null,
                "existed_before": true,
                "desired_absent": false,
                "order": 1
            }],
            "last_verification": null
        }))
        .unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_fighorse"))
        .args([
            "install",
            "skill",
            "--apply",
            "--home",
            install_home.to_str().unwrap(),
            "--clients",
            "cursor,codex,kimi",
        ])
        .env("HOME", &user_home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!cursor_legacy.exists());
    assert!(!codex_legacy.exists());
    assert!(!cursor_rule.exists());
    let manifest = load_manifest(&install_home).unwrap();
    let missing = manifest
        .managed_files
        .iter()
        .find(|file| file.path == missing_legacy)
        .unwrap();
    assert!(missing.desired_absent, "{missing:?}");
    assert!(missing.backup.is_none());
    let _ = fs::remove_dir_all(root);
}
