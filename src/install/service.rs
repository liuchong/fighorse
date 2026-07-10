//! Canonical launchd and systemd service renderers.

use super::model::InstallCheck;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

/// Injectable boundary for service-manager commands.
pub trait ServiceCommandRunner {
    fn run(&mut self, command: &str, args: &[String]) -> Result<Value>;
}

pub struct ProcessCommandRunner;

impl ServiceCommandRunner for ProcessCommandRunner {
    fn run(&mut self, command: &str, args: &[String]) -> Result<Value> {
        let output = Command::new(command).args(args).output()?;
        Ok(json!({
            "command": command,
            "args": args,
            "ok": output.status.success(),
            "exit_code": output.status.code().unwrap_or(1),
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
        }))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceProcessState {
    pub loaded: bool,
    pub enabled: bool,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceState {
    pub manager: String,
    pub target: PathBuf,
    #[serde(default)]
    pub existed_before: bool,
    #[serde(default)]
    pub before: ServiceProcessState,
}

impl ServiceState {
    pub fn new(manager: impl Into<String>, target: PathBuf, existed_before: bool) -> Self {
        Self {
            manager: manager.into(),
            target,
            existed_before,
            before: ServiceProcessState {
                loaded: existed_before,
                enabled: existed_before,
                running: existed_before,
            },
        }
    }

    pub fn captured(
        manager: impl Into<String>,
        target: PathBuf,
        existed_before: bool,
        before: ServiceProcessState,
    ) -> Self {
        Self {
            manager: manager.into(),
            target,
            existed_before,
            before,
        }
    }
}

pub fn probe_service_state(
    runner: &mut dyn ServiceCommandRunner,
    manager: &str,
    target: PathBuf,
) -> Result<ServiceState> {
    let existed_before = std::fs::symlink_metadata(&target).is_ok();
    let before = match manager {
        "launchd" => probe_launchd(runner)?,
        "systemd" => probe_systemd(runner)?,
        other => {
            return Err(crate::error::Error::Usage(format!(
                "Unsupported service manager: {other}. Expected launchd or systemd."
            )))
        }
    };
    Ok(ServiceState::captured(
        manager,
        target,
        existed_before,
        before,
    ))
}

fn probe_launchd(runner: &mut dyn ServiceCommandRunner) -> Result<ServiceProcessState> {
    let uid = runner.run("id", &strings(&["-u"]))?;
    if uid.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(crate::error::Error::Other(format!(
            "launchd_uid probe failed: {uid}"
        )));
    }
    let uid = uid
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let domain = if uid.is_empty() {
        "gui".to_string()
    } else {
        format!("gui/{uid}")
    };
    let label = format!("{domain}/com.groupultra.fighorse.mcp");
    let print = runner.run("launchctl", &strings(&["print", &label]))?;
    let loaded = command_ok(&print);
    let running = loaded
        && print
            .get("stdout")
            .and_then(Value::as_str)
            .is_some_and(|stdout| stdout.contains("state = running"));
    let disabled = runner.run("launchctl", &strings(&["print-disabled", &domain]))?;
    let disabled_text = disabled
        .get("stdout")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let explicitly_disabled = disabled_text
        .lines()
        .any(|line| line.contains("com.groupultra.fighorse.mcp") && line.contains("true"));
    Ok(ServiceProcessState {
        loaded,
        enabled: !explicitly_disabled,
        running,
    })
}

fn probe_systemd(runner: &mut dyn ServiceCommandRunner) -> Result<ServiceProcessState> {
    let loaded = runner.run(
        "systemctl",
        &strings(&[
            "--user",
            "show",
            "--property=LoadState",
            "--value",
            "fighorse-mcp.service",
        ]),
    )?;
    let enabled = runner.run(
        "systemctl",
        &strings(&["--user", "is-enabled", "fighorse-mcp.service"]),
    )?;
    let running = runner.run(
        "systemctl",
        &strings(&["--user", "is-active", "fighorse-mcp.service"]),
    )?;
    Ok(ServiceProcessState {
        loaded: command_ok(&loaded)
            && loaded.get("stdout").and_then(Value::as_str).map(str::trim) == Some("loaded"),
        enabled: command_ok(&enabled)
            && enabled.get("stdout").and_then(Value::as_str).map(str::trim) == Some("enabled"),
        running: command_ok(&running)
            && running.get("stdout").and_then(Value::as_str).map(str::trim) == Some("active"),
    })
}

pub fn activate_service(
    runner: &mut dyn ServiceCommandRunner,
    state: &ServiceState,
) -> Result<Value> {
    match state.manager.as_str() {
        "launchd" => {
            let uid = checked_run(runner, "id", &["-u"], "launchd_uid")?;
            let uid = uid
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let domain = if uid.is_empty() {
                "gui".to_string()
            } else {
                format!("gui/{uid}")
            };
            let target = state.target.to_string_lossy().into_owned();
            let bootout = runner.run("launchctl", &strings(&["bootout", &domain, &target]))?;
            let bootstrap = checked_run(
                runner,
                "launchctl",
                &["bootstrap", &domain, &target],
                "launchd_bootstrap",
            )?;
            let label = format!("{domain}/com.groupultra.fighorse.mcp");
            let kickstart = checked_run(
                runner,
                "launchctl",
                &["kickstart", "-k", &label],
                "launchd_kickstart",
            )?;
            Ok(json!({
                "bootout": bootout,
                "bootstrap": bootstrap,
                "kickstart": kickstart,
            }))
        }
        "systemd" => {
            let reload = checked_run(
                runner,
                "systemctl",
                &["--user", "daemon-reload"],
                "systemd_daemon_reload",
            )?;
            let enable = checked_run(
                runner,
                "systemctl",
                &["--user", "enable", "--now", "fighorse-mcp.service"],
                "systemd_enable",
            )?;
            Ok(json!({"daemon_reload": reload, "enable_now": enable}))
        }
        other => Err(crate::error::Error::Usage(format!(
            "Unsupported service manager: {other}. Expected launchd or systemd."
        ))),
    }
}

/// Reconcile the service process after managed files have already been
/// restored. Existing services are reloaded from the restored file; fresh
/// services are stopped and unloaded.
pub fn rollback_service(
    runner: &mut dyn ServiceCommandRunner,
    state: &ServiceState,
) -> Vec<InstallCheck> {
    match state.manager.as_str() {
        "launchd" => rollback_launchd(runner, state),
        "systemd" => rollback_systemd(runner, state),
        other => vec![InstallCheck::new(
            "service_rollback",
            false,
            format!("unsupported service manager: {other}"),
        )],
    }
}

fn rollback_launchd(
    runner: &mut dyn ServiceCommandRunner,
    state: &ServiceState,
) -> Vec<InstallCheck> {
    let uid = runner.run("id", &strings(&["-u"]));
    let domain = uid
        .as_ref()
        .ok()
        .and_then(|result| result.get("stdout"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|uid| !uid.is_empty())
        .map(|uid| format!("gui/{uid}"))
        .unwrap_or_else(|| "gui".into());
    let target = state.target.to_string_lossy().into_owned();
    let mut checks = vec![result_check(
        "service_rollback_launchd_uid",
        uid,
        "resolved launchd domain",
    )];
    checks.push(command_check(
        "service_rollback_launchd_bootout",
        runner.run("launchctl", &strings(&["bootout", &domain, &target])),
    ));
    if state.before.enabled {
        let label = format!("{domain}/com.groupultra.fighorse.mcp");
        checks.push(command_check(
            "service_rollback_launchd_enable",
            runner.run("launchctl", &strings(&["enable", &label])),
        ));
    } else {
        let label = format!("{domain}/com.groupultra.fighorse.mcp");
        checks.push(command_check(
            "service_rollback_launchd_disable",
            runner.run("launchctl", &strings(&["disable", &label])),
        ));
    }
    if state.before.loaded {
        checks.push(command_check(
            "service_rollback_launchd_bootstrap",
            runner.run("launchctl", &strings(&["bootstrap", &domain, &target])),
        ));
        let label = format!("{domain}/com.groupultra.fighorse.mcp");
        if state.before.running {
            checks.push(command_check(
                "service_rollback_launchd_kickstart",
                runner.run("launchctl", &strings(&["kickstart", "-k", &label])),
            ));
        } else {
            checks.push(command_check(
                "service_rollback_launchd_stop",
                runner.run("launchctl", &strings(&["stop", &label])),
            ));
        }
    }
    checks
}

fn rollback_systemd(
    runner: &mut dyn ServiceCommandRunner,
    state: &ServiceState,
) -> Vec<InstallCheck> {
    let mut checks = Vec::new();
    checks.push(command_check(
        "service_rollback_systemd_daemon_reload",
        runner.run("systemctl", &strings(&["--user", "daemon-reload"])),
    ));
    if state.before.enabled {
        checks.push(command_check(
            "service_rollback_systemd_enable",
            runner.run(
                "systemctl",
                &strings(&["--user", "enable", "fighorse-mcp.service"]),
            ),
        ));
    } else {
        checks.push(command_check(
            "service_rollback_systemd_disable",
            runner.run(
                "systemctl",
                &strings(&["--user", "disable", "fighorse-mcp.service"]),
            ),
        ));
    }
    if state.before.running {
        checks.push(command_check(
            "service_rollback_systemd_start",
            runner.run(
                "systemctl",
                &strings(&["--user", "start", "fighorse-mcp.service"]),
            ),
        ));
    } else {
        checks.push(command_check(
            "service_rollback_systemd_stop",
            runner.run(
                "systemctl",
                &strings(&["--user", "stop", "fighorse-mcp.service"]),
            ),
        ));
    }
    checks
}

fn checked_run(
    runner: &mut dyn ServiceCommandRunner,
    command: &str,
    args: &[&str],
    step: &str,
) -> Result<Value> {
    let result = runner.run(command, &strings(args))?;
    if result.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(crate::error::Error::Other(format!(
            "{step} failed: {result}"
        )));
    }
    Ok(result)
}

fn command_ok(value: &Value) -> bool {
    value.get("ok").and_then(Value::as_bool) == Some(true)
}

fn command_check(name: &str, result: Result<Value>) -> InstallCheck {
    match result {
        Ok(value) => InstallCheck::new(
            name,
            value.get("ok").and_then(Value::as_bool) == Some(true),
            value.to_string(),
        ),
        Err(error) => InstallCheck::new(name, false, error.to_string()),
    }
}

fn result_check(name: &str, result: Result<Value>, success: &str) -> InstallCheck {
    match result {
        Ok(value) if value.get("ok").and_then(Value::as_bool) == Some(true) => {
            InstallCheck::new(name, true, success)
        }
        Ok(value) => InstallCheck::new(name, false, value.to_string()),
        Err(error) => InstallCheck::new(name, false, error.to_string()),
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

/// Render the launchd service used by both review and apply paths.
pub fn launchd_plist(command: &str, port: i64, home: &str, allow_local_write: bool) -> String {
    let command = xml_escape(command);
    let home = xml_escape(home);
    let local_write = if allow_local_write { "allow" } else { "deny" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.groupultra.fighorse.mcp</string>
  <key>ProgramArguments</key>
  <array><string>{command}</string><string>mcp</string><string>serve</string><string>--transport</string><string>http</string><string>--host</string><string>127.0.0.1</string><string>--port</string><string>{port}</string></array>
  <key>EnvironmentVariables</key>
  <dict><key>FIGHORSE_HOME</key><string>{home}</string><key>FIGHORSE_MCP_MODE</key><string>readonly</string><key>FIGHORSE_MCP_LOCAL_WRITE</key><string>{local_write}</string><key>FIGHORSE_MCP_SERVICE</key><string>true</string></dict>
  <key>WorkingDirectory</key><string>{home}</string>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>{home}/logs/mcp.out.log</string>
  <key>StandardErrorPath</key><string>{home}/logs/mcp.err.log</string>
</dict>
</plist>
"#
    )
}

/// Render the systemd user service used by both review and apply paths.
pub fn systemd_unit(command: &str, port: i64, home: &str, allow_local_write: bool) -> String {
    let local_write = if allow_local_write { "allow" } else { "deny" };
    let command = systemd_quote(command);
    let home_value = systemd_escape(home);
    let home = systemd_quote(home);
    format!(
        "[Unit]\n\
Description=fighorse MCP service\n\
\n\
[Service]\n\
Environment=\"FIGHORSE_HOME={home_value}\"\n\
Environment=\"FIGHORSE_MCP_MODE=readonly\"\n\
Environment=\"FIGHORSE_MCP_LOCAL_WRITE={local_write}\"\n\
Environment=\"FIGHORSE_MCP_SERVICE=true\"\n\
ExecStart={command} mcp serve --transport http --host 127.0.0.1 --port {port}\n\
Restart=always\n\
WorkingDirectory={home}\n\
\n\
[Install]\n\
WantedBy=default.target\n"
    )
}

fn systemd_quote(value: &str) -> String {
    format!("\"{}\"", systemd_escape(value))
}

fn systemd_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
