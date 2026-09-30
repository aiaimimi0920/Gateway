//! Stable persistence contracts, shared state and the single-writer guard boundary.

mod archives;
mod atomic_json;
mod file_io;
mod first_save;
mod local_active;
mod paths;
mod platform;
mod records;
mod validation;
mod yaml;

pub(crate) use file_io::read_transaction_json;
use paths::{
    absolute_lexical, configure_lock_sharing, ensure_regular_file, ensure_secure_directory,
    reject_existing_links, reject_link_metadata, validate_platform_path,
    validate_unresolved_platform_path,
};
use validation::validate_safe_id;

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::revision::RevisionMetadata;
use crate::routing::config::RouteConfigYaml;

const STATE_VERSION: u32 = 1;
const FIRST_SAVE_STATUS_FILE: &str = "routes-first-save.json";
const FIRST_SAVE_BACKUP_FILE: &str = "routes-first-save.yaml";
const WRITER_LOCK_FILE: &str = "writer.lock";

pub const REPLACE_FILE_FLAGS: u32 = 0;
pub const MOVEFILE_WRITE_THROUGH_FLAG: u32 = 0x8;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct PersistenceError {
    code: &'static str,
    message: String,
}

pub type ConsolePersistenceError = PersistenceError;

impl PersistenceError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) fn io(operation: &str, error: impl fmt::Display) -> Self {
        Self::new(
            "console_persistence_io",
            format!("Gateway console persistence failed during {operation}: {error}"),
        )
    }

    pub(crate) fn corruption(message: impl Into<String>) -> Self {
        Self::new("console_corruption", message)
    }

    pub(crate) fn recovery_required(message: impl Into<String>) -> Self {
        Self::new("console_recovery_required", message)
    }

    pub(crate) fn invalid_phase(message: impl Into<String>) -> Self {
        Self::new("console_invalid_phase", message)
    }

    pub(crate) fn path_escape(message: impl Into<String>) -> Self {
        Self::new("console_journal_path_escape", message)
    }
}

pub trait AtomicReplaceBackend: Send + Sync {
    fn replace_existing(
        &self,
        destination: &Path,
        replacement: &Path,
        backup: &Path,
        flags: u32,
    ) -> io::Result<()>;

    fn move_new(&self, source: &Path, destination: &Path, flags: u32) -> io::Result<()>;
}

#[derive(Debug, Default)]
pub struct PlatformAtomicReplaceBackend;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FirstSavePresence {
    Present,
    Absent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FirstSaveStatus {
    version: u32,
    presence: FirstSavePresence,
    original_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YamlReplaceReceipt {
    previous_yaml_present: bool,
    previous_yaml_digest: Option<String>,
    new_yaml_digest: String,
    backup_leaf: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionArchive {
    path: PathBuf,
    relative_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct StoredRouteRevision {
    archive: RevisionArchive,
    metadata: RevisionMetadata,
    document: RouteConfigYaml,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransactionPhase {
    Prepared,
    YamlReplaced,
    RedisActivated,
    Committed,
    Aborted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransactionRecord {
    version: u32,
    tx_id: String,
    phase: TransactionPhase,
    old_revision: Option<String>,
    new_revision: String,
    archive_relative_path: String,
    previous_yaml_present: bool,
    previous_yaml_digest: Option<String>,
    new_yaml_digest: String,
    same_dir_backup_leaf: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
    abort_code: Option<String>,
    error_code: Option<String>,
}

pub struct WriterLockGuard {
    _file: File,
    state_root: PathBuf,
}

impl fmt::Debug for WriterLockGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriterLockGuard")
            .field("state_root", &self.state_root)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct RouteConfigPersistence {
    state_root: PathBuf,
    routes_path: PathBuf,
    backend: Arc<dyn AtomicReplaceBackend>,
    read_only: Arc<AtomicBool>,
    degraded_code: Arc<Mutex<Option<&'static str>>>,
}

impl fmt::Debug for RouteConfigPersistence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteConfigPersistence")
            .field("state_root", &self.state_root)
            .field("routes_path", &self.routes_path)
            .field("read_only", &self.is_read_only())
            .finish_non_exhaustive()
    }
}

impl RouteConfigPersistence {
    pub fn new(state_dir: &Path, routes_path: &Path) -> Result<Self, PersistenceError> {
        Self::for_test(state_dir, routes_path)
    }

    pub fn for_test(state_dir: &Path, routes_path: &Path) -> Result<Self, PersistenceError> {
        Self::with_backend(
            state_dir,
            routes_path,
            Arc::new(PlatformAtomicReplaceBackend),
        )
    }

    pub fn with_backend(
        state_dir: &Path,
        routes_path: &Path,
        backend: Arc<dyn AtomicReplaceBackend>,
    ) -> Result<Self, PersistenceError> {
        validate_unresolved_platform_path(state_dir)?;
        validate_unresolved_platform_path(routes_path)?;
        let state_dir = absolute_lexical(state_dir)?;
        let routes_path = absolute_lexical(routes_path)?;
        validate_platform_path(&state_dir)?;
        validate_platform_path(&routes_path)?;
        reject_existing_links(&state_dir)?;
        reject_existing_links(&routes_path)?;

        let state_root = state_dir.join("console");
        ensure_secure_directory(&state_root)?;
        for directory in [
            state_root.join("revisions"),
            state_root.join("transactions"),
            state_root.join("backups"),
        ] {
            ensure_secure_directory(&directory)?;
        }
        let writer_lock = state_root.join(WRITER_LOCK_FILE);
        ensure_regular_file(&writer_lock)?;

        Ok(Self {
            state_root,
            routes_path,
            backend,
            read_only: Arc::new(AtomicBool::new(false)),
            degraded_code: Arc::new(Mutex::new(None)),
        })
    }

    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn routes_path(&self) -> &Path {
        &self.routes_path
    }

    pub fn revisions_dir(&self) -> PathBuf {
        self.state_root.join("revisions")
    }

    pub fn transactions_dir(&self) -> PathBuf {
        self.state_root.join("transactions")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.state_root.join("backups")
    }

    pub fn writer_lock_path(&self) -> PathBuf {
        self.state_root.join(WRITER_LOCK_FILE)
    }

    pub fn journal_path(&self) -> PathBuf {
        self.state_root.join("journal.ndjson")
    }

    pub fn first_save_backup_path(&self) -> PathBuf {
        self.backups_dir().join(FIRST_SAVE_BACKUP_FILE)
    }

    pub fn first_save_status_path(&self) -> PathBuf {
        self.backups_dir().join(FIRST_SAVE_STATUS_FILE)
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only.load(Ordering::Acquire)
    }

    pub fn degraded_code(&self) -> Option<&'static str> {
        *self.degraded_code.lock()
    }

    pub fn try_writer_lock(&self) -> Result<WriterLockGuard, PersistenceError> {
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        configure_lock_sharing(&mut options);
        let file = options
            .open(self.writer_lock_path())
            .map_err(|error| PersistenceError::io("opening writer lock", error))?;
        file.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => PersistenceError::new(
                "console_locked",
                "Gateway console persistence is locked by another writer",
            ),
            std::fs::TryLockError::Error(error) => {
                PersistenceError::io("acquiring writer lock", error)
            }
        })?;
        Ok(WriterLockGuard {
            _file: file,
            state_root: self.state_root.clone(),
        })
    }

    pub(crate) fn ensure_guard(&self, guard: &WriterLockGuard) -> Result<(), PersistenceError> {
        if guard.state_root != self.state_root {
            return Err(PersistenceError::new(
                "console_locked",
                "Writer lock belongs to a different Gateway console state root",
            ));
        }
        Ok(())
    }

    pub(crate) fn ensure_writable(&self) -> Result<(), PersistenceError> {
        if self.is_read_only() {
            return Err(PersistenceError::recovery_required(
                "Gateway console persistence is degraded and read-only",
            ));
        }
        Ok(())
    }

    pub(crate) fn mark_degraded(&self, code: &'static str) {
        self.read_only.store(true, Ordering::Release);
        *self.degraded_code.lock() = Some(code);
    }

    pub(crate) fn transaction_path(&self, tx_id: &str) -> Result<PathBuf, PersistenceError> {
        validate_safe_id("transaction", tx_id)?;
        Ok(self.transactions_dir().join(format!("{tx_id}.json")))
    }

    pub fn transaction_record_path(&self, tx_id: &str) -> Result<PathBuf, PersistenceError> {
        self.transaction_path(tx_id)
    }

    pub(crate) fn validate_optional_managed_file(
        &self,
        path: &Path,
    ) -> Result<(), PersistenceError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                reject_link_metadata(path, &metadata)?;
                if !metadata.is_file() {
                    return Err(PersistenceError::corruption(format!(
                        "Managed console path '{}' is not a regular file",
                        path.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(PersistenceError::io("checking managed file", error)),
        }
        Ok(())
    }
}
