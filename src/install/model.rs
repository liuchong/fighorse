//! Stable data model for an installation transaction.

use super::clients::ClientKind;
use super::skills::SkillMigrationReport;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Canonical transaction stages. Their declaration order is not used for
/// execution; an [`InstallPlan`] carries the exact ordered sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStep {
    Preflight,
    Backup,
    Binary,
    Service,
    HealthReady,
    Clients,
    Skills,
    Verified,
}

/// Immutable description of the work an installer will perform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub mode: String,
    pub home: PathBuf,
    pub endpoint: Option<String>,
    pub clients: Vec<ClientKind>,
    pub steps: Vec<InstallStep>,
}

impl InstallPlan {
    pub fn cli(home: PathBuf) -> Self {
        Self {
            mode: "cli".into(),
            home,
            endpoint: None,
            clients: Vec::new(),
            steps: vec![
                InstallStep::Preflight,
                InstallStep::Backup,
                InstallStep::Binary,
                InstallStep::Skills,
                InstallStep::Verified,
            ],
        }
    }

    pub fn service(home: PathBuf, endpoint: impl Into<String>, clients: Vec<ClientKind>) -> Self {
        Self {
            mode: "service".into(),
            home,
            endpoint: Some(endpoint.into()),
            clients,
            steps: vec![
                InstallStep::Preflight,
                InstallStep::Backup,
                InstallStep::Binary,
                InstallStep::Service,
                InstallStep::HealthReady,
                InstallStep::Clients,
                InstallStep::Skills,
                InstallStep::Verified,
            ],
        }
    }
}

/// Result of one verification or rollback operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

impl InstallCheck {
    pub fn new(name: impl Into<String>, ok: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ok,
            detail: detail.into(),
        }
    }
}

/// Serializable transaction report. It deliberately has no token field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub plan: Option<InstallPlan>,
    pub completed: Vec<InstallStep>,
    pub verification: Vec<InstallCheck>,
    pub rollback: Vec<InstallCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_migration: Option<SkillMigrationReport>,
    pub ok: bool,
}

impl Default for InstallReport {
    fn default() -> Self {
        Self {
            plan: None,
            completed: Vec::new(),
            verification: Vec::new(),
            rollback: Vec::new(),
            skills_migration: None,
            ok: true,
        }
    }
}
