//! Canonical cross-client skill and rule locations.

use super::clients::ClientKind;
use super::transaction::InstallTransaction;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillTargetKind {
    Skill,
    CursorRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillTarget {
    pub kind: SkillTargetKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GeneratedSkillTemplates {
    known_contents: Vec<Vec<u8>>,
}

impl GeneratedSkillTemplates {
    pub fn new(
        skill: impl AsRef<[u8]>,
        agents: impl AsRef<[u8]>,
        cursor_rule: impl AsRef<[u8]>,
    ) -> Self {
        Self {
            known_contents: vec![
                skill.as_ref().to_vec(),
                agents.as_ref().to_vec(),
                cursor_rule.as_ref().to_vec(),
            ],
        }
    }

    pub fn with_edb26d2_templates(mut self) -> Self {
        let agents = include_bytes!("legacy/edb26d2-agents.md").to_vec();
        let skill = include_bytes!("legacy/edb26d2-skill.md").to_vec();
        let mut cursor_rule = b"---\ndescription: Use fighorse for Figma design replication\nalwaysApply: false\n---\n\n".to_vec();
        cursor_rule.extend_from_slice(&agents);
        self.known_contents.extend([skill, agents, cursor_rule]);
        self
    }

    fn recognizes(&self, content: &[u8]) -> bool {
        self.known_contents
            .iter()
            .any(|known| known.as_slice() == content)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMigrationConflict {
    pub path: PathBuf,
    pub backup: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMigrationReport {
    pub removed: Vec<PathBuf>,
    pub conflicts: Vec<SkillMigrationConflict>,
    pub backups: Vec<PathBuf>,
}

/// Resolve the deduplicated canonical locations for the selected clients.
pub fn canonical_targets(home: &Path, clients: &[ClientKind]) -> Vec<SkillTarget> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |kind, path: PathBuf| {
        if seen.insert(path.clone()) {
            targets.push(SkillTarget { kind, path });
        }
    };

    for client in clients {
        match client {
            ClientKind::Cursor | ClientKind::Kimi | ClientKind::Codex => push(
                SkillTargetKind::Skill,
                home.join(".agents")
                    .join("skills")
                    .join("fighorse")
                    .join("SKILL.md"),
            ),
            ClientKind::Claude => push(
                SkillTargetKind::Skill,
                home.join(".claude")
                    .join("skills")
                    .join("fighorse")
                    .join("SKILL.md"),
            ),
        }
        if *client == ClientKind::Cursor {
            push(
                SkillTargetKind::CursorRule,
                home.join(".cursor").join("rules").join("fighorse.mdc"),
            );
        }
    }
    targets
}

/// Remove only recognized generated legacy files. Unknown or customized
/// content is preserved in place and copied to a deterministic conflict
/// backup for review.
pub fn migrate_legacy(
    transaction: &mut InstallTransaction,
    home: &Path,
    templates: &GeneratedSkillTemplates,
) -> Result<SkillMigrationReport> {
    migrate_legacy_for_clients(
        transaction,
        home,
        &[
            ClientKind::Cursor,
            ClientKind::Kimi,
            ClientKind::Claude,
            ClientKind::Codex,
        ],
        templates,
    )
}

pub fn migrate_legacy_for_clients(
    transaction: &mut InstallTransaction,
    home: &Path,
    clients: &[ClientKind],
    templates: &GeneratedSkillTemplates,
) -> Result<SkillMigrationReport> {
    let mut report = SkillMigrationReport::default();
    for path in legacy_candidates(home, clients) {
        let content = match std::fs::read(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                transaction.remove_managed(&path)?;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        if templates.recognizes(&content) {
            if let Some(backup) = transaction.remove_managed(&path)? {
                report.backups.push(backup);
                report.removed.push(path);
            }
        } else {
            let backup = transaction.backup_conflict(&path)?;
            report.backups.push(backup.clone());
            report
                .conflicts
                .push(SkillMigrationConflict { path, backup });
        }
    }
    Ok(report)
}

fn legacy_candidates(home: &Path, clients: &[ClientKind]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let shared = clients.iter().any(|client| {
        matches!(
            client,
            ClientKind::Cursor | ClientKind::Kimi | ClientKind::Codex
        )
    });
    if shared {
        for root in [
            home.join(".cursor").join("skills").join("fighorse"),
            home.join(".codex").join("skills").join("fighorse"),
            home.join(".kimi").join("skills").join("fighorse"),
            home.join(".config")
                .join("agents")
                .join("skills")
                .join("fighorse"),
        ] {
            for name in ["SKILL.md", "AGENTS.md", "cursor-rule.mdc"] {
                candidates.push(root.join(name));
            }
        }
        let canonical = home.join(".agents").join("skills").join("fighorse");
        for name in ["AGENTS.md", "cursor-rule.mdc"] {
            candidates.push(canonical.join(name));
        }
    }
    if clients.contains(&ClientKind::Claude) {
        let canonical = home.join(".claude").join("skills").join("fighorse");
        for name in ["AGENTS.md", "cursor-rule.mdc"] {
            candidates.push(canonical.join(name));
        }
    }
    candidates
}
