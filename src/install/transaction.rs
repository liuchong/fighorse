//! Managed-file transaction, manifest, verification, and MCP readiness checks.

use super::clients::ClientSpec;
use super::model::{InstallCheck, InstallReport};
use super::service::{self, ServiceCommandRunner, ServiceState};
use crate::error::{Error, Result};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedFileKind {
    #[default]
    Regular,
    Symlink,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedFileVerification {
    #[default]
    Exact,
    ClientConfig {
        spec: ClientSpec,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedFile {
    pub path: PathBuf,
    pub hash: String,
    pub backup: Option<PathBuf>,
    pub existed_before: bool,
    #[serde(default)]
    pub desired_absent: bool,
    #[serde(default)]
    pub order: u64,
    #[serde(default)]
    pub kind: ManagedFileKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_kind: Option<ManagedFileKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_symlink_target: Option<PathBuf>,
    #[serde(default)]
    pub verification: ManagedFileVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallManifest {
    pub schema_version: u32,
    pub managed_files: Vec<ManagedFile>,
    pub last_verification: Option<Vec<InstallCheck>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceState>,
}

pub struct InstallTransaction {
    home: PathBuf,
    previous: BTreeMap<PathBuf, ManagedFile>,
    /// Final records to persist in the next manifest, including no-op updates.
    changed: BTreeMap<PathBuf, ManagedFile>,
    /// Filesystem mutations made in this transaction, with transaction-start
    /// snapshots retained for failure rollback.
    pending: BTreeMap<PathBuf, ManagedFile>,
    next_order: u64,
    endpoint: Option<String>,
    service: Option<ServiceState>,
}

struct PathSnapshot {
    kind: ManagedFileKind,
    content: Vec<u8>,
    mode: Option<u32>,
    symlink_target: Option<PathBuf>,
}

impl InstallTransaction {
    pub fn new(home: impl Into<PathBuf>) -> Result<Self> {
        let home = home.into();
        let prior_manifest = if manifest_path(&home).exists() {
            Some(load_manifest(&home)?)
        } else {
            None
        };
        let previous: BTreeMap<_, _> = prior_manifest
            .as_ref()
            .map(|manifest| {
                manifest
                    .managed_files
                    .iter()
                    .cloned()
                    .map(|file| (file.path.clone(), file))
                    .collect()
            })
            .unwrap_or_default();
        let next_order = previous
            .values()
            .map(|file| file.order)
            .max()
            .unwrap_or_default()
            + 1;
        Ok(Self {
            home,
            previous,
            changed: BTreeMap::new(),
            pending: BTreeMap::new(),
            next_order,
            endpoint: prior_manifest
                .as_ref()
                .and_then(|manifest| manifest.endpoint.clone()),
            service: prior_manifest.and_then(|manifest| manifest.service),
        })
    }

    pub fn set_endpoint(&mut self, endpoint: Option<String>) {
        self.endpoint = endpoint;
    }

    pub fn set_service(&mut self, service: Option<ServiceState>) {
        self.service = service;
    }

    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    pub fn service(&self) -> Option<&ServiceState> {
        self.service.as_ref()
    }

    pub fn write_managed(&mut self, path: &Path, content: &[u8]) -> Result<()> {
        let mode = snapshot_path(path)?.and_then(|snapshot| snapshot.mode);
        self.write_managed_with_mode(path, content, mode.unwrap_or(0o644))
    }

    pub fn write_managed_client_config(
        &mut self,
        path: &Path,
        content: &[u8],
        spec: &ClientSpec,
    ) -> Result<()> {
        self.write_managed(path, content)?;
        let managed = self.changed.get_mut(path).ok_or_else(|| {
            Error::Other(format!("managed client config missing: {}", path.display()))
        })?;
        managed.verification = ManagedFileVerification::ClientConfig { spec: spec.clone() };
        if let Some(pending) = self.pending.get_mut(path) {
            pending.verification = ManagedFileVerification::ClientConfig { spec: spec.clone() };
        }
        Ok(())
    }

    pub fn write_managed_with_mode(
        &mut self,
        path: &Path,
        content: &[u8],
        mode: u32,
    ) -> Result<()> {
        let snapshot = snapshot_path(path)?;
        if snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.kind == ManagedFileKind::Symlink)
        {
            return Err(Error::Other(format!(
                "refusing to follow or overwrite symbolic link: {}",
                path.display()
            )));
        }
        let desired_hash = content_hash(content);
        let current = snapshot
            .as_ref()
            .map(|snapshot| snapshot.content.as_slice());
        if current.is_some_and(|bytes| content_hash(bytes) == desired_hash) {
            let existing = self
                .changed
                .get(path)
                .cloned()
                .or_else(|| self.previous.get(path).cloned());
            let mut managed = match existing {
                Some(managed) if !managed.desired_absent => managed,
                Some(mut managed) => {
                    managed.backup = current
                        .map(|bytes| self.backup_file(path, bytes))
                        .transpose()?;
                    managed.existed_before = true;
                    managed.desired_absent = false;
                    managed.kind = ManagedFileKind::Regular;
                    apply_snapshot_metadata(&mut managed, snapshot.as_ref());
                    managed
                }
                None => {
                    let backup = current
                        .map(|bytes| self.backup_file(path, bytes))
                        .transpose()?;
                    let order = self.take_order();
                    let mut managed = ManagedFile {
                        path: path.to_path_buf(),
                        hash: desired_hash.clone(),
                        backup,
                        existed_before: true,
                        desired_absent: false,
                        order,
                        kind: ManagedFileKind::Regular,
                        previous_kind: None,
                        previous_mode: None,
                        previous_symlink_target: None,
                        verification: ManagedFileVerification::Exact,
                    };
                    apply_snapshot_metadata(&mut managed, snapshot.as_ref());
                    managed
                }
            };
            managed.hash = desired_hash;
            managed.desired_absent = false;
            managed.kind = ManagedFileKind::Regular;
            managed.verification = ManagedFileVerification::Exact;
            self.changed.insert(path.to_path_buf(), managed);
            return Ok(());
        }

        let mut pending = match self.pending.get(path).cloned() {
            Some(pending) => pending,
            None => {
                let backup = current
                    .map(|bytes| self.backup_file(path, bytes))
                    .transpose()?;
                let order = self.take_order();
                let mut pending = ManagedFile {
                    path: path.to_path_buf(),
                    hash: desired_hash.clone(),
                    backup,
                    existed_before: snapshot.is_some(),
                    desired_absent: false,
                    order,
                    kind: ManagedFileKind::Regular,
                    previous_kind: None,
                    previous_mode: None,
                    previous_symlink_target: None,
                    verification: ManagedFileVerification::Exact,
                };
                apply_snapshot_metadata(&mut pending, snapshot.as_ref());
                pending
            }
        };
        pending.hash = desired_hash.clone();
        pending.desired_absent = false;
        pending.kind = ManagedFileKind::Regular;
        pending.verification = ManagedFileVerification::Exact;

        let existing = self
            .changed
            .get(path)
            .cloned()
            .or_else(|| self.previous.get(path).cloned());
        let mut managed = match existing {
            Some(managed) if current.is_some_and(|bytes| managed.hash == content_hash(bytes)) => {
                managed
            }
            Some(managed) if self.changed.contains_key(path) => managed,
            previous_managed => {
                let backup = current
                    .map(|bytes| self.backup_file(path, bytes))
                    .transpose()?
                    .or_else(|| {
                        previous_managed
                            .as_ref()
                            .and_then(|file| file.backup.clone())
                    });
                let existed_before = current.is_some()
                    || previous_managed
                        .as_ref()
                        .is_some_and(|file| file.existed_before);
                let order = previous_managed
                    .as_ref()
                    .map(|file| file.order)
                    .unwrap_or_else(|| self.take_order());
                let mut managed = ManagedFile {
                    path: path.to_path_buf(),
                    hash: desired_hash.clone(),
                    backup,
                    existed_before,
                    desired_absent: false,
                    order,
                    kind: ManagedFileKind::Regular,
                    previous_kind: None,
                    previous_mode: None,
                    previous_symlink_target: None,
                    verification: ManagedFileVerification::Exact,
                };
                apply_snapshot_metadata(&mut managed, snapshot.as_ref());
                managed
            }
        };

        atomic_write_mode(path, content, mode)?;
        managed.hash = desired_hash;
        managed.desired_absent = false;
        managed.kind = ManagedFileKind::Regular;
        managed.verification = ManagedFileVerification::Exact;
        self.pending.insert(path.to_path_buf(), pending);
        self.changed.insert(path.to_path_buf(), managed);
        Ok(())
    }

    #[cfg(unix)]
    pub fn write_managed_symlink(&mut self, path: &Path, target: &Path) -> Result<()> {
        let snapshot = snapshot_path(path)?;
        let target_hash = content_hash(target.as_os_str().as_encoded_bytes());
        if snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.kind == ManagedFileKind::Symlink
                && snapshot.symlink_target.as_deref() == Some(target)
        }) {
            let existing = self
                .changed
                .get(path)
                .cloned()
                .or_else(|| self.previous.get(path).cloned());
            let mut managed = match existing {
                Some(managed) => managed,
                None => {
                    let order = self.take_order();
                    let mut managed = ManagedFile {
                        path: path.to_path_buf(),
                        hash: target_hash.clone(),
                        backup: None,
                        existed_before: true,
                        desired_absent: false,
                        order,
                        kind: ManagedFileKind::Symlink,
                        previous_kind: None,
                        previous_mode: None,
                        previous_symlink_target: None,
                        verification: ManagedFileVerification::Exact,
                    };
                    apply_snapshot_metadata(&mut managed, snapshot.as_ref());
                    managed
                }
            };
            managed.hash = target_hash;
            managed.kind = ManagedFileKind::Symlink;
            managed.desired_absent = false;
            managed.verification = ManagedFileVerification::Exact;
            self.changed.insert(path.to_path_buf(), managed);
            return Ok(());
        }

        let mut pending = match self.pending.get(path).cloned() {
            Some(pending) => pending,
            None => {
                let backup = snapshot
                    .as_ref()
                    .filter(|snapshot| snapshot.kind == ManagedFileKind::Regular)
                    .map(|snapshot| self.backup_file(path, &snapshot.content))
                    .transpose()?;
                let order = self.take_order();
                let mut pending = ManagedFile {
                    path: path.to_path_buf(),
                    hash: target_hash.clone(),
                    backup,
                    existed_before: snapshot.is_some(),
                    desired_absent: false,
                    order,
                    kind: ManagedFileKind::Symlink,
                    previous_kind: None,
                    previous_mode: None,
                    previous_symlink_target: None,
                    verification: ManagedFileVerification::Exact,
                };
                apply_snapshot_metadata(&mut pending, snapshot.as_ref());
                pending
            }
        };
        pending.hash = target_hash.clone();
        pending.desired_absent = false;
        pending.kind = ManagedFileKind::Symlink;

        let existing = self
            .changed
            .get(path)
            .cloned()
            .or_else(|| self.previous.get(path).cloned());
        let mut managed = match existing {
            Some(managed) if self.changed.contains_key(path) => managed,
            Some(managed)
                if snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot_matches(snapshot, &managed)) =>
            {
                managed
            }
            previous_managed => {
                let backup = snapshot
                    .as_ref()
                    .filter(|snapshot| snapshot.kind == ManagedFileKind::Regular)
                    .map(|snapshot| self.backup_file(path, &snapshot.content))
                    .transpose()?;
                let existed_before = snapshot.is_some()
                    || previous_managed
                        .as_ref()
                        .is_some_and(|file| file.existed_before);
                let order = previous_managed
                    .as_ref()
                    .map(|file| file.order)
                    .unwrap_or_else(|| self.take_order());
                let mut managed = ManagedFile {
                    path: path.to_path_buf(),
                    hash: target_hash.clone(),
                    backup,
                    existed_before,
                    desired_absent: false,
                    order,
                    kind: ManagedFileKind::Symlink,
                    previous_kind: None,
                    previous_mode: None,
                    previous_symlink_target: None,
                    verification: ManagedFileVerification::Exact,
                };
                apply_snapshot_metadata(&mut managed, snapshot.as_ref());
                managed
            }
        };
        atomic_symlink(path, target)?;
        managed.hash = target_hash;
        managed.desired_absent = false;
        managed.kind = ManagedFileKind::Symlink;
        managed.verification = ManagedFileVerification::Exact;
        self.pending.insert(path.to_path_buf(), pending);
        self.changed.insert(path.to_path_buf(), managed);
        Ok(())
    }

    /// Remove a managed path while retaining enough state to restore it.
    /// Returns the rollback backup only when a file was removed by this call.
    pub fn remove_managed(&mut self, path: &Path) -> Result<Option<PathBuf>> {
        let snapshot = match snapshot_path(path)? {
            Some(snapshot) => snapshot,
            None => {
                if self.pending.contains_key(path) {
                    return Ok(None);
                }
                let existing = self
                    .changed
                    .get(path)
                    .cloned()
                    .or_else(|| self.previous.get(path).cloned());
                if let Some(mut managed) = existing.filter(|file| !file.desired_absent) {
                    managed.hash.clear();
                    managed.backup = None;
                    managed.existed_before = false;
                    managed.desired_absent = true;
                    self.changed.insert(path.to_path_buf(), managed);
                }
                return Ok(None);
            }
        };
        let pending_backup = if snapshot.kind == ManagedFileKind::Regular {
            Some(self.backup_file(path, &snapshot.content)?)
        } else {
            None
        };
        let mut pending = match self.pending.get(path).cloned() {
            Some(pending) => pending,
            None => {
                let order = self.take_order();
                let mut pending = ManagedFile {
                    path: path.to_path_buf(),
                    hash: snapshot_hash(&snapshot),
                    backup: pending_backup.clone(),
                    existed_before: true,
                    desired_absent: true,
                    order,
                    kind: snapshot.kind,
                    previous_kind: None,
                    previous_mode: None,
                    previous_symlink_target: None,
                    verification: ManagedFileVerification::Exact,
                };
                apply_snapshot_metadata(&mut pending, Some(&snapshot));
                pending
            }
        };
        pending.hash = snapshot_hash(&snapshot);
        pending.desired_absent = true;

        let existing = self
            .changed
            .get(path)
            .cloned()
            .or_else(|| self.previous.get(path).cloned());
        let mut managed = match existing {
            Some(managed) if self.changed.contains_key(path) => managed,
            previous_managed => {
                let backup = if snapshot.kind == ManagedFileKind::Regular {
                    Some(self.backup_file(path, &snapshot.content)?)
                } else {
                    None
                };
                let order = previous_managed
                    .as_ref()
                    .map(|file| file.order)
                    .unwrap_or_else(|| self.take_order());
                let mut managed = ManagedFile {
                    path: path.to_path_buf(),
                    hash: snapshot_hash(&snapshot),
                    backup,
                    existed_before: true,
                    desired_absent: true,
                    order,
                    kind: snapshot.kind,
                    previous_kind: None,
                    previous_mode: None,
                    previous_symlink_target: None,
                    verification: ManagedFileVerification::Exact,
                };
                apply_snapshot_metadata(&mut managed, Some(&snapshot));
                managed
            }
        };
        std::fs::remove_file(path)?;
        managed.hash = snapshot_hash(&snapshot);
        managed.desired_absent = true;
        managed.kind = snapshot.kind;
        managed.verification = ManagedFileVerification::Exact;
        self.pending.insert(path.to_path_buf(), pending);
        self.changed.insert(path.to_path_buf(), managed);
        Ok(pending_backup)
    }

    /// Create an idempotent safety copy for an unmanaged conflict.
    pub fn backup_conflict(&self, path: &Path) -> Result<PathBuf> {
        let bytes = std::fs::read(path)?;
        let name = format!(
            "conflict-{}-{}-{}.bak",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("managed"),
            &content_hash(path.to_string_lossy().as_bytes())[..12],
            &content_hash(&bytes)[..12],
        );
        let backup = self.home.join("install").join("backups").join(name);
        if !backup.exists() {
            atomic_write_mode(&backup, &bytes, 0o600)?;
        }
        Ok(backup)
    }

    pub fn commit(&self, last_verification: Option<Vec<InstallCheck>>) -> Result<InstallManifest> {
        let mut files: BTreeMap<_, _> = self
            .previous
            .iter()
            .filter(|(path, file)| {
                if self.changed.contains_key(*path) || file.desired_absent {
                    return true;
                }
                match std::fs::symlink_metadata(path) {
                    Ok(_) => true,
                    Err(error) => error.kind() != std::io::ErrorKind::NotFound,
                }
            })
            .map(|(path, file)| (path.clone(), file.clone()))
            .collect();
        files.extend(self.changed.clone());
        let manifest = InstallManifest {
            schema_version: 4,
            managed_files: files.into_values().collect(),
            last_verification,
            endpoint: self.endpoint.clone(),
            service: self.service.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&manifest)?;
        secure_dir(&self.home.join("install"))?;
        secure_dir(&self.home.join("install").join("backups"))?;
        atomic_write_mode(&manifest_path(&self.home), &bytes, 0o600)?;
        Ok(manifest)
    }

    /// Restore writes performed by this in-memory transaction. This is used
    /// when a later install stage fails before the manifest can be committed.
    pub fn rollback_pending(&self) -> Vec<InstallCheck> {
        rollback_files(self.pending.values())
    }

    /// Verify only paths written or removed by this transaction.
    ///
    /// Targeted installers must not fail because an unrelated path from an
    /// older manifest was customized after its original installation.
    pub fn verify_changed(&self) -> Vec<InstallCheck> {
        self.pending.values().map(verify_managed_file).collect()
    }

    pub fn rollback_pending_with_service(
        &self,
        runner: &mut dyn ServiceCommandRunner,
        service_touched: bool,
    ) -> Vec<InstallCheck> {
        let mut checks = self.rollback_pending();
        if service_touched {
            match self.service.as_ref() {
                Some(state) => checks.extend(service::rollback_service(runner, state)),
                None => checks.push(InstallCheck::new(
                    "service_rollback",
                    false,
                    "service was touched without captured prior state",
                )),
            }
        }
        checks
    }

    fn backup_file(&self, path: &Path, bytes: &[u8]) -> Result<PathBuf> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!(
            "{stamp}-{}-{}.bak",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("managed"),
            &content_hash(path.to_string_lossy().as_bytes())[..12]
        );
        let backup = self.home.join("install").join("backups").join(name);
        secure_dir(&self.home.join("install"))?;
        secure_dir(&self.home.join("install").join("backups"))?;
        atomic_write_mode(&backup, bytes, 0o600)?;
        Ok(backup)
    }

    fn take_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order += 1;
        order
    }
}

pub fn manifest_path(home: &Path) -> PathBuf {
    home.join("install").join("manifest.json")
}

pub fn load_manifest(home: &Path) -> Result<InstallManifest> {
    let text = std::fs::read_to_string(manifest_path(home))?;
    Ok(serde_json::from_str(&text)?)
}

pub fn verify_manifest(home: &Path) -> Result<Vec<InstallCheck>> {
    let manifest = load_manifest(home)?;
    Ok(manifest
        .managed_files
        .iter()
        .map(verify_managed_file)
        .collect())
}

fn verify_managed_file(managed: &ManagedFile) -> InstallCheck {
    match snapshot_path(&managed.path) {
        Ok(None) if managed.desired_absent => InstallCheck::new(
            format!("managed:{}", managed.path.display()),
            true,
            "managed path is absent as expected",
        ),
        Ok(Some(_)) if managed.desired_absent => InstallCheck::new(
            format!("managed:{}", managed.path.display()),
            false,
            "managed path should be absent",
        ),
        Ok(Some(snapshot)) => {
            let matches = match &managed.verification {
                ManagedFileVerification::Exact => snapshot_matches(&snapshot, managed),
                ManagedFileVerification::ClientConfig { spec } => {
                    snapshot.kind == ManagedFileKind::Regular
                        && String::from_utf8(snapshot.content)
                            .is_ok_and(|content| spec.matches_config(&content))
                }
            };
            InstallCheck::new(
                format!("managed:{}", managed.path.display()),
                matches,
                if matches {
                    "managed content matches manifest"
                } else {
                    "managed content or file type differs from manifest"
                },
            )
        }
        Ok(None) => InstallCheck::new(
            format!("managed:{}", managed.path.display()),
            false,
            "managed path is absent",
        ),
        Err(error) => InstallCheck::new(
            format!("managed:{}", managed.path.display()),
            false,
            error.to_string(),
        ),
    }
}

pub fn rollback(home: &Path) -> Result<InstallReport> {
    let mut runner = service::ProcessCommandRunner;
    rollback_with_runner(home, &mut runner)
}

pub fn rollback_with_runner(
    home: &Path,
    runner: &mut dyn ServiceCommandRunner,
) -> Result<InstallReport> {
    let manifest = load_manifest(home)?;
    let mut rollback = rollback_files(manifest.managed_files.iter());
    if let Some(state) = manifest.service.as_ref() {
        rollback.extend(service::rollback_service(runner, state));
    }
    Ok(InstallReport {
        ok: rollback.iter().all(|item| item.ok),
        rollback,
        ..InstallReport::default()
    })
}

fn rollback_files<'a>(files: impl IntoIterator<Item = &'a ManagedFile>) -> Vec<InstallCheck> {
    let mut files: Vec<_> = files.into_iter().collect();
    files.sort_by_key(|file| std::cmp::Reverse(file.order));
    files.into_iter().map(restore_managed_file).collect()
}

fn restore_managed_file(managed: &ManagedFile) -> InstallCheck {
    let name = format!("rollback:{}", managed.path.display());
    if managed.desired_absent {
        return match snapshot_path(&managed.path) {
            Ok(Some(_)) => InstallCheck::new(
                name,
                false,
                "skipped because the removed managed file was recreated",
            ),
            Ok(None) if managed.existed_before => match restore_previous(managed) {
                Ok(()) => InstallCheck::new(name, true, "restored removed managed file"),
                Err(error) => InstallCheck::new(name, false, error.to_string()),
            },
            Ok(None) => InstallCheck::new(name, true, "managed path remains absent"),
            Err(error) => InstallCheck::new(name, false, error.to_string()),
        };
    }
    let current = match snapshot_path(&managed.path) {
        Ok(Some(snapshot)) if snapshot_matches(&snapshot, managed) => Some(snapshot),
        Ok(Some(_)) => {
            return InstallCheck::new(
                name,
                false,
                "skipped because the managed file has user changes",
            );
        }
        Ok(None) => None,
        Err(error) => return InstallCheck::new(name, false, error.to_string()),
    };

    if current.is_none() && managed.existed_before {
        return InstallCheck::new(name, false, "managed file was removed after installation");
    }
    match (managed.existed_before, current.is_some()) {
        (true, true) => match restore_previous(managed) {
            Ok(()) => InstallCheck::new(name, true, "restored managed backup"),
            Err(error) => InstallCheck::new(name, false, error.to_string()),
        },
        (false, true) => match std::fs::remove_file(&managed.path) {
            Ok(()) => InstallCheck::new(name, true, "removed installer-created file"),
            Err(error) => InstallCheck::new(name, false, error.to_string()),
        },
        (_, false) => InstallCheck::new(name, true, "already absent"),
    }
}

/// Poll `/health`, then complete a real MCP initialize and tools/list
/// handshake. This is intentionally async so production callers never create
/// a nested runtime.
pub async fn wait_for_mcp_ready(
    endpoint: &str,
    attempts: usize,
    delay: Duration,
) -> Result<Vec<InstallCheck>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let base = endpoint
        .strip_suffix("/mcp")
        .ok_or_else(|| Error::Usage("MCP endpoint must end with /mcp.".into()))?;
    let health_url = format!("{base}/health");

    let mut healthy = false;
    for _ in 0..attempts.max(1) {
        if client
            .get(&health_url)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            healthy = true;
            break;
        }
        tokio::time::sleep(delay).await;
    }
    if !healthy {
        return Err(Error::Other(format!(
            "Installed MCP service did not become healthy at {health_url}"
        )));
    }

    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "fighorse-installer", "version": env!("CARGO_PKG_VERSION")}
        }
    });
    let initialized = mcp_post(&client, endpoint, &initialize, None).await?;
    let session = initialized.0;
    if initialized.1["result"]["serverInfo"]["name"] != "fighorse" {
        return Err(Error::Other(
            "MCP initialize response did not identify fighorse.".into(),
        ));
    }

    let notification = json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}});
    let _ = mcp_post(&client, endpoint, &notification, session.as_deref()).await?;
    let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    let listed = mcp_post(&client, endpoint, &list, session.as_deref()).await?;
    if listed.1["result"]["tools"]
        .as_array()
        .is_none_or(|tools| tools.is_empty())
    {
        return Err(Error::Other(
            "MCP tools/list returned no fighorse tools.".into(),
        ));
    }
    Ok(vec![
        InstallCheck::new("service_health", true, health_url),
        InstallCheck::new("mcp_initialize", true, "fighorse"),
        InstallCheck::new("mcp_tools_list", true, "tools available"),
    ])
}

async fn mcp_post(
    client: &reqwest::Client,
    endpoint: &str,
    message: &Value,
    session: Option<&str>,
) -> Result<(Option<String>, Value)> {
    let mut request = client
        .post(endpoint)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .json(message);
    if let Some(session) = session {
        request = request.header("mcp-session-id", session);
    }
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(Error::Other(format!(
            "MCP request failed with status {}",
            response.status()
        )));
    }
    let session = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let is_sse = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"));
    let text = response.text().await?;
    let body = if text.trim().is_empty() {
        Value::Null
    } else if is_sse {
        let data = text
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .find(|line| !line.is_empty());
        match data {
            Some(data) => serde_json::from_str(data)?,
            None if message.get("id").is_none() => Value::Null,
            None => {
                return Err(Error::Other(
                    "MCP SSE response contained no data event.".into(),
                ));
            }
        }
    } else {
        serde_json::from_str(&text)?
    };
    Ok((session, body))
}

fn content_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn snapshot_path(path: &Path) -> std::io::Result<Option<PathSnapshot>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mode = file_mode(&metadata);
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(path)?;
        return Ok(Some(PathSnapshot {
            kind: ManagedFileKind::Symlink,
            content: Vec::new(),
            mode,
            symlink_target: Some(target),
        }));
    }
    if !metadata.file_type().is_file() {
        return Err(std::io::Error::other(format!(
            "managed path is not a regular file or symbolic link: {}",
            path.display()
        )));
    }
    Ok(Some(PathSnapshot {
        kind: ManagedFileKind::Regular,
        content: std::fs::read(path)?,
        mode,
        symlink_target: None,
    }))
}

fn snapshot_hash(snapshot: &PathSnapshot) -> String {
    match snapshot.kind {
        ManagedFileKind::Regular => content_hash(&snapshot.content),
        ManagedFileKind::Symlink => content_hash(
            snapshot
                .symlink_target
                .as_ref()
                .map(|target| target.as_os_str().as_encoded_bytes())
                .unwrap_or_default(),
        ),
    }
}

fn snapshot_matches(snapshot: &PathSnapshot, managed: &ManagedFile) -> bool {
    snapshot.kind == managed.kind && snapshot_hash(snapshot) == managed.hash
}

fn apply_snapshot_metadata(managed: &mut ManagedFile, snapshot: Option<&PathSnapshot>) {
    managed.previous_kind = snapshot.map(|snapshot| snapshot.kind);
    managed.previous_mode = snapshot.and_then(|snapshot| snapshot.mode);
    managed.previous_symlink_target = snapshot.and_then(|snapshot| snapshot.symlink_target.clone());
}

fn restore_previous(managed: &ManagedFile) -> std::io::Result<()> {
    match managed.previous_kind.unwrap_or(ManagedFileKind::Regular) {
        ManagedFileKind::Regular => {
            let backup = managed.backup.as_ref().ok_or_else(|| {
                std::io::Error::other("managed backup is missing for pre-existing file")
            })?;
            let bytes = std::fs::read(backup)?;
            if std::fs::symlink_metadata(&managed.path)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                std::fs::remove_file(&managed.path)?;
            }
            atomic_write_io(
                &managed.path,
                &bytes,
                managed.previous_mode.unwrap_or(0o600),
            )
        }
        ManagedFileKind::Symlink => {
            let target = managed.previous_symlink_target.as_ref().ok_or_else(|| {
                std::io::Error::other("managed symlink target is missing from manifest")
            })?;
            atomic_symlink(&managed.path, target)
        }
    }
}

#[cfg(unix)]
fn file_mode(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn file_mode(_metadata: &std::fs::Metadata) -> Option<u32> {
    None
}

fn secure_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn atomic_write_mode(path: &Path, content: &[u8], mode: u32) -> Result<()> {
    atomic_write_io(path, content, mode)?;
    Ok(())
}

fn atomic_write_io(path: &Path, content: &[u8], mode: u32) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(std::io::Error::other(format!(
            "refusing to overwrite symbolic link: {}",
            path.display()
        )));
    }
    let temporary = path.with_extension(format!(
        "{}.tmp-{}-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or(""),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        use std::io::Write;
        let mut file = options.open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(mode))?;
        }
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(unix)]
fn atomic_symlink(path: &Path, target: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!(
        "{}.tmp-link-{}-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or(""),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::os::unix::fs::symlink(target, &temporary)?;
    let result = std::fs::rename(&temporary, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(unix))]
fn atomic_symlink(_path: &Path, _target: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "symbolic links are not supported by this installer build",
    ))
}
