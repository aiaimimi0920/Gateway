use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use super::document::CanonicalRouteDocument;
use super::revision::RevisionMetadata;

const STATE_VERSION: u32 = 1;
const FIRST_SAVE_STATUS_FILE: &str = "routes-first-save.json";
const FIRST_SAVE_BACKUP_FILE: &str = "routes-first-save.yaml";
const WRITER_LOCK_FILE: &str = "writer.lock";
const MAX_METADATA_BYTES: u64 = 1024 * 1024;

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

#[cfg(windows)]
impl AtomicReplaceBackend for PlatformAtomicReplaceBackend {
    fn replace_existing(
        &self,
        destination: &Path,
        replacement: &Path,
        backup: &Path,
        flags: u32,
    ) -> io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

        if flags != REPLACE_FILE_FLAGS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ReplaceFileW flags must be zero",
            ));
        }

        let destination = windows_path(destination)?;
        let replacement = windows_path(replacement)?;
        let backup = windows_path(backup)?;
        let result = unsafe {
            ReplaceFileW(
                destination.as_ptr(),
                replacement.as_ptr(),
                backup.as_ptr(),
                flags,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn move_new(&self, source: &Path, destination: &Path, flags: u32) -> io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

        if flags != MOVEFILE_WRITE_THROUGH_FLAG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MoveFileExW must use MOVEFILE_WRITE_THROUGH without REPLACE_EXISTING",
            ));
        }

        let source = windows_path(source)?;
        let destination = windows_path(destination)?;
        let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(windows))]
impl AtomicReplaceBackend for PlatformAtomicReplaceBackend {
    fn replace_existing(
        &self,
        destination: &Path,
        replacement: &Path,
        _backup: &Path,
        flags: u32,
    ) -> io::Result<()> {
        if flags != REPLACE_FILE_FLAGS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "POSIX replacement flags must be zero",
            ));
        }
        fs::rename(replacement, destination)
    }

    fn move_new(&self, source: &Path, destination: &Path, flags: u32) -> io::Result<()> {
        if flags != MOVEFILE_WRITE_THROUGH_FLAG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "POSIX new-file move flags must be MOVEFILE_WRITE_THROUGH",
            ));
        }
        fs::rename(source, destination)
    }
}

#[cfg(windows)]
fn windows_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let raw = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let mut encoded = if raw.starts_with(&[b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16]) {
        raw
    } else if raw.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        let mut value = r"\\?\UNC\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&raw[2..]);
        value
    } else {
        let mut value = r"\\?\".encode_utf16().collect::<Vec<_>>();
        value.extend_from_slice(&raw);
        value
    };
    if encoded.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows path contains an interior NUL",
        ));
    }
    encoded.push(0);
    Ok(encoded)
}

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

impl FirstSaveStatus {
    pub fn presence(&self) -> FirstSavePresence {
        self.presence
    }

    pub fn original_digest(&self) -> Option<&str> {
        self.original_digest.as_deref()
    }

    fn validate(&self) -> Result<(), PersistenceError> {
        if self.version != STATE_VERSION {
            return Err(PersistenceError::corruption(
                "Unsupported first-save status version",
            ));
        }
        match self.presence {
            FirstSavePresence::Present if !is_lowercase_digest(self.original_digest.as_deref()) => {
                Err(PersistenceError::corruption(
                    "Present first-save status has an invalid original digest",
                ))
            }
            FirstSavePresence::Absent if self.original_digest.is_some() => {
                Err(PersistenceError::corruption(
                    "Absent first-save status unexpectedly contains a digest",
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YamlReplaceReceipt {
    previous_yaml_present: bool,
    previous_yaml_digest: Option<String>,
    new_yaml_digest: String,
    backup_leaf: Option<String>,
}

impl YamlReplaceReceipt {
    pub fn previous_yaml_present(&self) -> bool {
        self.previous_yaml_present
    }

    pub fn previous_yaml_digest(&self) -> Option<&str> {
        self.previous_yaml_digest.as_deref()
    }

    pub fn new_yaml_digest(&self) -> &str {
        &self.new_yaml_digest
    }

    pub fn backup_leaf(&self) -> Option<&str> {
        self.backup_leaf.as_deref()
    }

    pub(crate) fn from_record_parts(
        previous_yaml_present: bool,
        previous_yaml_digest: Option<&str>,
        new_yaml_digest: &str,
        backup_leaf: Option<&str>,
    ) -> Self {
        Self {
            previous_yaml_present,
            previous_yaml_digest: previous_yaml_digest.map(str::to_string),
            new_yaml_digest: new_yaml_digest.to_string(),
            backup_leaf: backup_leaf.map(str::to_string),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionArchive {
    path: PathBuf,
    relative_path: PathBuf,
}

impl RevisionArchive {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }
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

impl TransactionRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new_prepared(
        tx_id: impl Into<String>,
        old_revision: Option<&str>,
        new_revision: impl Into<String>,
        archive_relative_path: impl Into<String>,
        previous_yaml_present: bool,
        previous_yaml_digest: Option<&str>,
        new_yaml_digest: impl Into<String>,
        created_at: OffsetDateTime,
    ) -> Result<Self, PersistenceError> {
        let record = Self {
            version: STATE_VERSION,
            tx_id: tx_id.into(),
            phase: TransactionPhase::Prepared,
            old_revision: old_revision.map(str::to_string),
            new_revision: new_revision.into(),
            archive_relative_path: archive_relative_path.into(),
            previous_yaml_present,
            previous_yaml_digest: previous_yaml_digest.map(str::to_string),
            new_yaml_digest: new_yaml_digest.into(),
            same_dir_backup_leaf: previous_yaml_present
                .then(|| format!(".gateway-console-{}-bak", Uuid::new_v4())),
            created_at,
            updated_at: created_at,
            abort_code: None,
            error_code: None,
        };
        record.validate()?;
        Ok(record)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_prepared_with_backup_leaf(
        tx_id: impl Into<String>,
        old_revision: Option<&str>,
        new_revision: impl Into<String>,
        archive_relative_path: impl Into<String>,
        previous_yaml_present: bool,
        previous_yaml_digest: Option<&str>,
        new_yaml_digest: impl Into<String>,
        backup_leaf: Option<&str>,
        created_at: OffsetDateTime,
    ) -> Result<Self, PersistenceError> {
        let mut record = Self::new_prepared(
            tx_id,
            old_revision,
            new_revision,
            archive_relative_path,
            previous_yaml_present,
            previous_yaml_digest,
            new_yaml_digest,
            created_at,
        )?;
        record.set_prepared_backup_leaf(backup_leaf)?;
        Ok(record)
    }

    pub fn prepared(
        tx_id: impl Into<String>,
        archive_relative_path: impl Into<String>,
        old_revision: impl Into<String>,
        new_revision: impl Into<String>,
    ) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            version: STATE_VERSION,
            tx_id: tx_id.into(),
            phase: TransactionPhase::Prepared,
            old_revision: Some(old_revision.into()),
            new_revision: new_revision.into(),
            archive_relative_path: archive_relative_path.into(),
            previous_yaml_present: false,
            previous_yaml_digest: None,
            new_yaml_digest: "0".repeat(64),
            same_dir_backup_leaf: None,
            created_at: now,
            updated_at: now,
            abort_code: None,
            error_code: None,
        }
    }

    pub fn tx_id(&self) -> &str {
        &self.tx_id
    }

    pub fn phase(&self) -> TransactionPhase {
        self.phase
    }

    pub fn old_revision(&self) -> Option<&str> {
        self.old_revision.as_deref()
    }

    pub fn new_revision(&self) -> &str {
        &self.new_revision
    }

    pub fn archive_relative_path(&self) -> &str {
        &self.archive_relative_path
    }

    pub fn previous_yaml_present(&self) -> bool {
        self.previous_yaml_present
    }

    pub fn previous_yaml_digest(&self) -> Option<&str> {
        self.previous_yaml_digest.as_deref()
    }

    pub fn new_yaml_digest(&self) -> &str {
        &self.new_yaml_digest
    }

    pub fn same_dir_backup_leaf(&self) -> Option<&str> {
        self.same_dir_backup_leaf.as_deref()
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }

    pub fn abort_code(&self) -> Option<&str> {
        self.abort_code.as_deref()
    }

    pub fn error_code(&self) -> Option<&str> {
        self.error_code.as_deref()
    }

    pub fn record_yaml_replaced(
        &mut self,
        receipt: &YamlReplaceReceipt,
        now: OffsetDateTime,
    ) -> Result<(), PersistenceError> {
        if self.phase != TransactionPhase::Prepared {
            return Err(PersistenceError::invalid_phase(
                "YAML replacement details can only be recorded from Prepared",
            ));
        }
        if self.previous_yaml_present != receipt.previous_yaml_present
            || self.previous_yaml_digest != receipt.previous_yaml_digest
            || self.new_yaml_digest != receipt.new_yaml_digest
            || self.same_dir_backup_leaf != receipt.backup_leaf
        {
            return Err(PersistenceError::new(
                "console_revision_conflict",
                "YAML replacement receipt does not match the prepared transaction record",
            ));
        }
        self.transition(TransactionPhase::YamlReplaced, now, None)
    }

    pub(crate) fn set_prepared_backup_leaf(
        &mut self,
        backup_leaf: Option<&str>,
    ) -> Result<(), PersistenceError> {
        if self.phase != TransactionPhase::Prepared {
            return Err(PersistenceError::invalid_phase(
                "Prepared backup leaf cannot change after YAML replacement",
            ));
        }
        if let Some(leaf) = backup_leaf {
            validate_safe_leaf(leaf)?;
        }
        self.same_dir_backup_leaf = backup_leaf.map(str::to_string);
        self.validate()
    }

    pub fn transition(
        &mut self,
        next: TransactionPhase,
        now: OffsetDateTime,
        error_code: Option<&str>,
    ) -> Result<(), PersistenceError> {
        if let Some(code) = error_code {
            validate_error_code(code)?;
            if next != TransactionPhase::Aborted {
                return Err(PersistenceError::invalid_phase(
                    "Only an Aborted transaction may carry an error code",
                ));
            }
        }
        if next == self.phase {
            if error_code.is_some_and(|code| self.error_code.as_deref() != Some(code)) {
                return Err(PersistenceError::invalid_phase(
                    "Idempotent phase replay changed the error code",
                ));
            }
            return Ok(());
        }
        let allowed = matches!(
            (self.phase, next),
            (TransactionPhase::Prepared, TransactionPhase::YamlReplaced)
                | (
                    TransactionPhase::YamlReplaced,
                    TransactionPhase::RedisActivated
                )
                | (
                    TransactionPhase::RedisActivated,
                    TransactionPhase::Committed
                )
                | (TransactionPhase::Prepared, TransactionPhase::Aborted)
                | (TransactionPhase::YamlReplaced, TransactionPhase::Aborted)
        );
        if !allowed {
            return Err(PersistenceError::invalid_phase(format!(
                "Invalid transaction phase transition {:?} -> {:?}",
                self.phase, next
            )));
        }
        if now < self.updated_at {
            return Err(PersistenceError::invalid_phase(
                "Transaction timestamps cannot move backwards",
            ));
        }
        self.phase = next;
        self.updated_at = now;
        self.abort_code = (next == TransactionPhase::Aborted)
            .then(|| error_code.unwrap_or("local_recovery").to_string());
        self.error_code = error_code.map(str::to_string);
        self.validate()
    }

    pub fn validate(&self) -> Result<(), PersistenceError> {
        validate_archive_relative_path(&self.archive_relative_path)?;
        if self.version != STATE_VERSION {
            return Err(PersistenceError::corruption(
                "Unsupported transaction record version",
            ));
        }
        validate_safe_id("transaction", &self.tx_id)?;
        validate_revision_id(&self.new_revision)?;
        if let Some(old_revision) = self.old_revision.as_deref() {
            validate_revision_id(old_revision)?;
            if revision_sequence(old_revision) >= revision_sequence(&self.new_revision) {
                return Err(PersistenceError::corruption(
                    "Old revision must precede the new revision",
                ));
            }
        }
        let expected_archive = format!("revisions/{}", self.new_revision);
        if normalized_relative_string(Path::new(&self.archive_relative_path)) != expected_archive {
            return Err(PersistenceError::corruption(
                "Transaction archive path does not match the new revision",
            ));
        }
        if !is_lowercase_digest(Some(&self.new_yaml_digest)) {
            return Err(PersistenceError::corruption(
                "Transaction new YAML digest is invalid",
            ));
        }
        match (
            self.previous_yaml_present,
            self.previous_yaml_digest.as_deref(),
        ) {
            (true, digest) if !is_lowercase_digest(digest) => {
                return Err(PersistenceError::corruption(
                    "Transaction previous YAML digest is invalid",
                ));
            }
            (false, Some(_)) => {
                return Err(PersistenceError::corruption(
                    "Absent previous YAML cannot have a digest",
                ));
            }
            _ => {}
        }
        if let Some(leaf) = self.same_dir_backup_leaf.as_deref() {
            validate_safe_leaf(leaf)?;
        }
        if !self.previous_yaml_present && self.same_dir_backup_leaf.is_some() {
            return Err(PersistenceError::corruption(
                "Absent previous YAML cannot have a backup leaf",
            ));
        }
        if self.previous_yaml_present && self.same_dir_backup_leaf.is_none() {
            return Err(PersistenceError::corruption(
                "Present previous YAML must reserve a same-directory backup leaf",
            ));
        }
        if matches!(
            self.phase,
            TransactionPhase::YamlReplaced
                | TransactionPhase::RedisActivated
                | TransactionPhase::Committed
        ) && self.previous_yaml_present
            && self.same_dir_backup_leaf.is_none()
        {
            return Err(PersistenceError::corruption(
                "Replaced YAML transaction is missing its same-directory backup",
            ));
        }
        if self.updated_at < self.created_at {
            return Err(PersistenceError::corruption(
                "Transaction updated timestamp precedes creation",
            ));
        }
        if self.phase == TransactionPhase::Aborted {
            if self.abort_code.as_deref().is_none_or(str::is_empty) {
                return Err(PersistenceError::corruption(
                    "Aborted transaction is missing an abort code",
                ));
            }
            if self
                .error_code
                .as_deref()
                .is_some_and(|code| Some(code) != self.abort_code.as_deref())
            {
                return Err(PersistenceError::corruption(
                    "Aborted transaction error code must match its abort code",
                ));
            }
        } else if self.abort_code.is_some() || self.error_code.is_some() {
            return Err(PersistenceError::corruption(
                "Non-aborted transaction unexpectedly contains an abort or error code",
            ));
        }
        if let Some(code) = self.abort_code.as_deref() {
            validate_error_code(code)?;
        }
        if let Some(code) = self.error_code.as_deref() {
            validate_error_code(code)?;
        }
        Ok(())
    }
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

    pub fn ensure_first_save_backup(&self) -> Result<FirstSaveStatus, PersistenceError> {
        self.ensure_writable()?;
        let guard = self.try_writer_lock()?;
        self.ensure_first_save_backup_locked(&guard)
    }

    pub fn replace_routes_yaml(
        &self,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_writable()?;
        validate_canonical_yaml(canonical_yaml)?;
        let guard = self.try_writer_lock()?;
        self.replace_routes_yaml_locked(&guard, canonical_yaml)
    }

    pub fn replace_routes_yaml_for_transaction(
        &self,
        record: &TransactionRecord,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_writable()?;
        let guard = self.try_writer_lock()?;
        self.replace_routes_yaml_for_transaction_locked(&guard, record, canonical_yaml)
    }

    pub(crate) fn replace_routes_yaml_for_transaction_locked(
        &self,
        guard: &WriterLockGuard,
        record: &TransactionRecord,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        record.validate()?;
        if record.phase() != TransactionPhase::Prepared {
            return Err(PersistenceError::invalid_phase(
                "A transaction YAML replacement must start in Prepared",
            ));
        }
        validate_canonical_yaml(canonical_yaml)?;
        let expected_digest = sha256_hex(canonical_yaml);
        if expected_digest != record.new_yaml_digest() {
            return Err(PersistenceError::corruption(
                "Transaction candidate YAML digest does not match its record",
            ));
        }
        self.ensure_prepared_previous_matches_locked(guard, record)?;
        self.replace_routes_yaml_locked_with_backup_leaf(
            guard,
            canonical_yaml,
            record.same_dir_backup_leaf(),
        )
    }

    pub fn restore_routes_yaml(
        &self,
        receipt: &YamlReplaceReceipt,
    ) -> Result<(), PersistenceError> {
        let guard = self.try_writer_lock()?;
        self.restore_routes_yaml_locked(&guard, receipt)
    }

    pub fn archive_revision(
        &self,
        metadata: &RevisionMetadata,
        document: &CanonicalRouteDocument,
    ) -> Result<RevisionArchive, PersistenceError> {
        self.ensure_writable()?;
        let guard = self.try_writer_lock()?;
        self.archive_revision_locked(&guard, metadata, document)
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

    pub(crate) fn verify_transaction_archive(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), PersistenceError> {
        record.validate()?;
        let relative = Path::new(record.archive_relative_path());
        validate_archive_relative_path(record.archive_relative_path())?;
        let archive = self.state_root.join(relative);
        let metadata = fs::symlink_metadata(&archive).map_err(|error| {
            PersistenceError::corruption(format!(
                "Transaction revision archive is unavailable: {error}"
            ))
        })?;
        reject_link_metadata(&archive, &metadata)?;
        if !metadata.is_dir() {
            return Err(PersistenceError::corruption(
                "Transaction revision archive is not a directory",
            ));
        }
        let expected_files = vec![
            "document.json".to_string(),
            "metadata.json".to_string(),
            "routes.yaml".to_string(),
        ];
        let mut actual_files = Vec::new();
        for entry in fs::read_dir(&archive)
            .map_err(|error| PersistenceError::io("listing revision archive", error))?
        {
            let entry = entry
                .map_err(|error| PersistenceError::io("reading revision archive entry", error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| PersistenceError::io("checking revision archive entry", error))?;
            reject_link_metadata(&path, &metadata)?;
            if !metadata.is_file() {
                return Err(PersistenceError::corruption(
                    "Revision archive contains a non-file entry",
                ));
            }
            actual_files.push(entry.file_name().to_string_lossy().into_owned());
        }
        actual_files.sort();
        if actual_files != expected_files {
            return Err(PersistenceError::corruption(
                "Revision archive contains missing or unexpected files",
            ));
        }
        let document = read_regular_file(&archive.join("document.json"))?;
        let yaml = read_regular_file(&archive.join("routes.yaml"))?;
        let metadata_bytes = read_regular_file(&archive.join("metadata.json"))?;
        let metadata: RevisionMetadata = serde_json::from_slice(&metadata_bytes)
            .map_err(|_| PersistenceError::corruption("Revision metadata JSON is malformed"))?;
        let canonical_metadata = serde_json::to_vec(&metadata)
            .map_err(|error| PersistenceError::io("serializing revision metadata", error))?;
        if metadata.id() != record.new_revision()
            || metadata.parent() != record.old_revision()
            || metadata.yaml_digest() != record.new_yaml_digest()
            || sha256_hex(&yaml) != record.new_yaml_digest()
            || sha256_hex(&document) != metadata.document_digest()
            || metadata_bytes != canonical_metadata
        {
            return Err(PersistenceError::corruption(
                "Transaction revision archive digest or identity mismatch",
            ));
        }
        Ok(())
    }

    pub(crate) fn ensure_first_save_backup_locked(
        &self,
        guard: &WriterLockGuard,
    ) -> Result<FirstSaveStatus, PersistenceError> {
        self.ensure_guard(guard)?;
        let status_path = self.first_save_status_path();
        let backup_path = self.first_save_backup_path();

        if status_path.exists() {
            let status: FirstSaveStatus = read_json_limited(&status_path)?;
            status.validate()?;
            self.verify_first_save_files(&status)?;
            return Ok(status);
        }

        if backup_path.exists() {
            let backup = read_regular_file(&backup_path)?;
            let current = read_regular_file(&self.routes_path).map_err(|_| {
                PersistenceError::corruption(
                    "First-save backup exists without status or matching routes file",
                )
            })?;
            if backup != current {
                return Err(PersistenceError::corruption(
                    "First-save backup exists without status and differs from routes YAML",
                ));
            }
            let status = FirstSaveStatus {
                version: STATE_VERSION,
                presence: FirstSavePresence::Present,
                original_digest: Some(sha256_hex(&backup)),
            };
            create_new_json(&status_path, &status)?;
            sync_directory(self.backups_dir().as_path())?;
            return Ok(status);
        }

        let status = match fs::symlink_metadata(&self.routes_path) {
            Ok(metadata) => {
                reject_link_metadata(&self.routes_path, &metadata)?;
                if !metadata.is_file() {
                    return Err(PersistenceError::corruption(
                        "Configured routes path is not a regular file",
                    ));
                }
                let original = read_regular_file(&self.routes_path)?;
                create_new_bytes(&backup_path, &original)?;
                FirstSaveStatus {
                    version: STATE_VERSION,
                    presence: FirstSavePresence::Present,
                    original_digest: Some(sha256_hex(&original)),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => FirstSaveStatus {
                version: STATE_VERSION,
                presence: FirstSavePresence::Absent,
                original_digest: None,
            },
            Err(error) => return Err(PersistenceError::io("reading routes metadata", error)),
        };

        create_new_json(&status_path, &status)?;
        sync_directory(self.backups_dir().as_path())?;
        Ok(status)
    }

    fn verify_first_save_files(&self, status: &FirstSaveStatus) -> Result<(), PersistenceError> {
        let backup_path = self.first_save_backup_path();
        match status.presence {
            FirstSavePresence::Present => {
                let backup = read_regular_file(&backup_path).map_err(|_| {
                    PersistenceError::corruption(
                        "Present first-save status is missing its immutable backup",
                    )
                })?;
                if status.original_digest.as_deref() != Some(sha256_hex(&backup).as_str()) {
                    return Err(PersistenceError::corruption(
                        "First-save backup digest does not match its status",
                    ));
                }
            }
            FirstSavePresence::Absent if backup_path.exists() => {
                return Err(PersistenceError::corruption(
                    "Absent first-save status must not have a backup file",
                ));
            }
            FirstSavePresence::Absent => {}
        }
        Ok(())
    }

    pub(crate) fn replace_routes_yaml_locked(
        &self,
        guard: &WriterLockGuard,
        canonical_yaml: &[u8],
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        validate_canonical_yaml(canonical_yaml)?;
        reject_existing_links(&self.routes_path)?;
        self.ensure_first_save_backup_locked(guard)?;
        self.replace_routes_yaml_locked_with_backup_leaf(guard, canonical_yaml, None)
    }

    fn replace_routes_yaml_locked_with_backup_leaf(
        &self,
        guard: &WriterLockGuard,
        canonical_yaml: &[u8],
        forced_backup_leaf: Option<&str>,
    ) -> Result<YamlReplaceReceipt, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        validate_canonical_yaml(canonical_yaml)?;
        reject_existing_links(&self.routes_path)?;
        self.ensure_first_save_backup_locked(guard)?;

        let parent = self.routes_path.parent().ok_or_else(|| {
            PersistenceError::new(
                "console_unsupported_platform_operation",
                "Configured routes path has no parent directory",
            )
        })?;
        let parent_metadata = fs::symlink_metadata(parent)
            .map_err(|error| PersistenceError::io("reading routes parent", error))?;
        reject_link_metadata(parent, &parent_metadata)?;
        if !parent_metadata.is_dir() {
            return Err(PersistenceError::corruption(
                "Configured routes parent is not a directory",
            ));
        }

        let (previous_yaml_present, previous_yaml_digest) =
            match fs::symlink_metadata(&self.routes_path) {
                Ok(metadata) => {
                    reject_link_metadata(&self.routes_path, &metadata)?;
                    if !metadata.is_file() {
                        return Err(PersistenceError::corruption(
                            "Configured routes path is not a regular file",
                        ));
                    }
                    let bytes = read_regular_file(&self.routes_path)?;
                    (true, Some(sha256_hex(&bytes)))
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => (false, None),
                Err(error) => return Err(PersistenceError::io("reading routes metadata", error)),
            };

        let temp_path = write_same_dir_temp(&self.routes_path, "yaml", canonical_yaml)?;
        let backup_path = if previous_yaml_present {
            if let Some(leaf) = forced_backup_leaf {
                validate_safe_leaf(leaf)?;
                Some(self.routes_path.parent().unwrap().join(leaf))
            } else {
                Some(unique_sibling(&self.routes_path, "bak"))
            }
        } else {
            None
        };
        let replace_result = if let Some(backup_path) = backup_path.as_deref() {
            self.replace_existing_with_backup(&self.routes_path, &temp_path, backup_path)
        } else {
            self.backend
                .move_new(&temp_path, &self.routes_path, MOVEFILE_WRITE_THROUGH_FLAG)
        };
        if let Err(error) = replace_result {
            // Keep the same-directory temp/backup evidence. Recovery must inspect an
            // interrupted platform operation rather than guessing or deleting the target.
            self.mark_degraded("console_recovery_required");
            return Err(PersistenceError::io(
                "atomically replacing routes YAML",
                error,
            ));
        }
        sync_directory(parent)?;

        let persisted = read_regular_file(&self.routes_path)?;
        let new_yaml_digest = sha256_hex(canonical_yaml);
        if persisted != canonical_yaml {
            self.mark_degraded("console_corruption");
            return Err(PersistenceError::corruption(
                "Atomic routes replacement produced unexpected bytes",
            ));
        }
        if let (Some(backup_path), Some(expected_digest)) =
            (backup_path.as_deref(), previous_yaml_digest.as_deref())
        {
            let backup = read_regular_file(backup_path)?;
            if sha256_hex(&backup) != expected_digest {
                self.mark_degraded("console_corruption");
                return Err(PersistenceError::corruption(
                    "Same-directory routes backup does not match the previous YAML digest",
                ));
            }
        }

        Ok(YamlReplaceReceipt {
            previous_yaml_present,
            previous_yaml_digest,
            new_yaml_digest,
            backup_leaf: backup_path
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|leaf| leaf.to_string_lossy().into_owned()),
        })
    }

    fn ensure_prepared_previous_matches_locked(
        &self,
        guard: &WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        let actual = match fs::symlink_metadata(&self.routes_path) {
            Ok(metadata) => {
                reject_link_metadata(&self.routes_path, &metadata)?;
                if !metadata.is_file() {
                    return Err(PersistenceError::recovery_required(
                        "Prepared transaction routes path is not a regular file",
                    ));
                }
                let bytes = read_regular_file(&self.routes_path)?;
                (true, Some(sha256_hex(&bytes)))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (false, None),
            Err(error) => return Err(PersistenceError::io("reading prepared routes state", error)),
        };
        if actual.0 != record.previous_yaml_present()
            || actual.1.as_deref() != record.previous_yaml_digest()
        {
            return Err(PersistenceError::new(
                "console_revision_conflict",
                "Prepared transaction previous YAML no longer matches the recorded state",
            ));
        }
        Ok(())
    }

    pub(crate) fn restore_routes_yaml_locked(
        &self,
        guard: &WriterLockGuard,
        receipt: &YamlReplaceReceipt,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        let current = match fs::symlink_metadata(&self.routes_path) {
            Ok(metadata) => {
                reject_link_metadata(&self.routes_path, &metadata)?;
                Some(read_regular_file(&self.routes_path)?)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(PersistenceError::io("reading routes metadata", error)),
        };

        if receipt.previous_yaml_present {
            let previous_digest = receipt.previous_yaml_digest.as_deref().ok_or_else(|| {
                PersistenceError::corruption("Present previous YAML is missing its digest")
            })?;
            if current
                .as_deref()
                .is_some_and(|bytes| sha256_hex(bytes) == previous_digest)
            {
                return Ok(());
            }
            let current = current.ok_or_else(|| {
                PersistenceError::recovery_required(
                    "Routes YAML disappeared before the previous file could be restored",
                )
            })?;
            if sha256_hex(&current) != receipt.new_yaml_digest {
                return Err(PersistenceError::recovery_required(
                    "Routes YAML digest changed after replacement; recovery refused to overwrite it",
                ));
            }
            let leaf = receipt.backup_leaf.as_deref().ok_or_else(|| {
                PersistenceError::corruption("Present previous YAML is missing its backup leaf")
            })?;
            validate_safe_leaf(leaf)?;
            let backup_path = self.routes_path.parent().unwrap().join(leaf);
            let previous = read_regular_file(&backup_path)?;
            if sha256_hex(&previous) != previous_digest {
                return Err(PersistenceError::corruption(
                    "Routes recovery backup digest does not match the transaction record",
                ));
            }
            let temp_path = write_same_dir_temp(&self.routes_path, "restore", &previous)?;
            let displaced = unique_sibling(&self.routes_path, "rollback-bak");
            self.replace_existing_with_backup(&self.routes_path, &temp_path, &displaced)
                .map_err(|error| PersistenceError::recovery_required(error.to_string()))?;
            if let Err(error) = fs::remove_file(&displaced) {
                self.mark_degraded("console_recovery_required");
                return Err(PersistenceError::recovery_required(format!(
                    "Restored routes YAML but could not remove displaced recovery backup: {error}"
                )));
            }
            sync_directory(self.routes_path.parent().unwrap())?;
            if sha256_hex(&read_regular_file(&self.routes_path)?) != previous_digest {
                self.mark_degraded("console_corruption");
                return Err(PersistenceError::corruption(
                    "Restored routes YAML does not match the previous digest",
                ));
            }
            Ok(())
        } else {
            let Some(current) = current else {
                return Ok(());
            };
            if sha256_hex(&current) != receipt.new_yaml_digest {
                return Err(PersistenceError::recovery_required(
                    "Routes YAML digest changed after first managed write; recovery refused to remove it",
                ));
            }
            fs::remove_file(&self.routes_path).map_err(|error| {
                PersistenceError::recovery_required(format!(
                    "Failed to restore absent routes state: {error}"
                ))
            })?;
            sync_directory(self.routes_path.parent().unwrap())?;
            Ok(())
        }
    }

    pub(crate) fn archive_revision_locked(
        &self,
        guard: &WriterLockGuard,
        metadata: &RevisionMetadata,
        document: &CanonicalRouteDocument,
    ) -> Result<RevisionArchive, PersistenceError> {
        self.ensure_guard(guard)?;
        self.ensure_writable()?;
        metadata
            .validate()
            .map_err(|error| PersistenceError::corruption(error.to_string()))?;
        validate_canonical_yaml(document.canonical_yaml())?;
        if metadata.document_digest() != document.document_digest()
            || metadata.yaml_digest() != document.yaml_digest()
        {
            return Err(PersistenceError::corruption(
                "Revision metadata digests do not match the canonical document",
            ));
        }
        validate_revision_id(metadata.id())?;

        let relative_path = PathBuf::from("revisions").join(metadata.id());
        validate_archive_relative_path(&normalized_relative_string(&relative_path))?;
        let archive_path = self.state_root.join(&relative_path);
        let metadata_bytes = serde_json::to_vec(metadata)
            .map_err(|error| PersistenceError::io("serializing revision metadata", error))?;
        let expected = [
            ("document.json", document.canonical_json()),
            ("routes.yaml", document.canonical_yaml()),
            ("metadata.json", metadata_bytes.as_slice()),
        ];

        match fs::create_dir(&archive_path) {
            Ok(()) => {
                for (leaf, bytes) in expected {
                    create_new_bytes(&archive_path.join(leaf), bytes)?;
                }
                sync_directory(&archive_path)?;
                sync_directory(self.revisions_dir().as_path())?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&archive_path)
                    .map_err(|error| PersistenceError::io("reading revision archive", error))?;
                reject_link_metadata(&archive_path, &metadata)?;
                if !metadata.is_dir() {
                    return Err(PersistenceError::corruption(
                        "Revision archive path is not a directory",
                    ));
                }
                let mut actual_files = Vec::new();
                for entry in fs::read_dir(&archive_path)
                    .map_err(|error| PersistenceError::io("listing revision archive", error))?
                {
                    let entry = entry.map_err(|error| {
                        PersistenceError::io("reading revision archive entry", error)
                    })?;
                    let path = entry.path();
                    let metadata = fs::symlink_metadata(&path).map_err(|error| {
                        PersistenceError::io("checking revision archive entry", error)
                    })?;
                    reject_link_metadata(&path, &metadata)?;
                    if !metadata.is_file() {
                        return Err(PersistenceError::corruption(
                            "Existing revision archive contains a non-file entry",
                        ));
                    }
                    actual_files.push(entry.file_name().to_string_lossy().into_owned());
                }
                actual_files.sort();
                if actual_files
                    != [
                        "document.json".to_string(),
                        "metadata.json".to_string(),
                        "routes.yaml".to_string(),
                    ]
                {
                    return Err(PersistenceError::corruption(
                        "Existing revision archive contains missing or unexpected entries",
                    ));
                }
                for (leaf, bytes) in expected {
                    let existing = read_regular_file(&archive_path.join(leaf)).map_err(|_| {
                        PersistenceError::corruption(
                            "Existing revision archive is incomplete or unreadable",
                        )
                    })?;
                    if existing != bytes {
                        return Err(PersistenceError::corruption(
                            "Existing revision archive differs from the immutable revision",
                        ));
                    }
                }
            }
            Err(error) => {
                return Err(PersistenceError::io("creating revision archive", error));
            }
        }

        Ok(RevisionArchive {
            path: archive_path,
            relative_path,
        })
    }

    pub(crate) fn atomic_write_json_locked<T: Serialize>(
        &self,
        guard: &WriterLockGuard,
        destination: &Path,
        value: &T,
    ) -> Result<(), PersistenceError> {
        self.ensure_guard(guard)?;
        let mut bytes = serde_json::to_vec(value)
            .map_err(|error| PersistenceError::io("serializing transaction JSON", error))?;
        bytes.push(b'\n');
        let parent = destination.parent().ok_or_else(|| {
            PersistenceError::new(
                "console_unsupported_platform_operation",
                "Transaction path has no parent directory",
            )
        })?;
        match fs::symlink_metadata(destination) {
            Ok(metadata) => {
                reject_link_metadata(destination, &metadata)?;
                if !metadata.is_file() {
                    return Err(PersistenceError::corruption(
                        "Transaction record path is not a regular file",
                    ));
                }
                let temp_path = write_same_dir_temp(destination, "json", &bytes)?;
                let backup = unique_sibling(destination, "json-bak");
                if let Err(error) =
                    self.replace_existing_with_backup(destination, &temp_path, &backup)
                {
                    self.mark_degraded("console_recovery_required");
                    return Err(PersistenceError::recovery_required(format!(
                        "Transaction JSON replacement has uncertain durable state: {error}"
                    )));
                }
                if let Err(error) = fs::remove_file(&backup) {
                    self.mark_degraded("console_recovery_required");
                    return Err(PersistenceError::recovery_required(format!(
                        "Transaction JSON updated but its temporary backup could not be removed: {error}"
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let temp_path = write_same_dir_temp(destination, "json", &bytes)?;
                if let Err(error) =
                    self.backend
                        .move_new(&temp_path, destination, MOVEFILE_WRITE_THROUGH_FLAG)
                {
                    self.mark_degraded("console_recovery_required");
                    return Err(PersistenceError::io(
                        "creating transaction JSON atomically",
                        error,
                    ));
                }
            }
            Err(error) => return Err(PersistenceError::io("reading transaction metadata", error)),
        }
        sync_directory(parent)?;
        Ok(())
    }

    fn replace_existing_with_backup(
        &self,
        destination: &Path,
        replacement: &Path,
        backup: &Path,
    ) -> io::Result<()> {
        if destination.parent() != replacement.parent() || destination.parent() != backup.parent() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "atomic replacement files must share one parent directory",
            ));
        }
        match fs::symlink_metadata(backup) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "atomic replacement backup path is a link/reparse point",
                    ));
                }
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "atomic replacement backup path already exists",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        #[cfg(not(windows))]
        {
            let previous = read_regular_file_io(destination)?;
            create_new_bytes_io(backup, &previous)?;
        }
        self.backend
            .replace_existing(destination, replacement, backup, REPLACE_FILE_FLAGS)
    }
}

fn validate_canonical_yaml(bytes: &[u8]) -> Result<(), PersistenceError> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF])
        || bytes.contains(&b'\r')
        || std::str::from_utf8(bytes).is_err()
    {
        return Err(PersistenceError::new(
            "console_persistence_invalid_yaml",
            "Canonical routes YAML must be UTF-8 without BOM and use LF line endings",
        ));
    }
    Ok(())
}

fn absolute_lexical(path: &Path) -> Result<PathBuf, PersistenceError> {
    let absolute = std::path::absolute(path)
        .map_err(|error| PersistenceError::io("resolving an absolute path", error))?;
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(PersistenceError::path_escape(
                        "Mutable console path escapes its filesystem root",
                    ));
                }
            }
        }
    }
    Ok(normalized)
}

#[cfg(windows)]
fn validate_platform_path(path: &Path) -> Result<(), PersistenceError> {
    use std::path::Prefix;

    let mut saw_prefix = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                saw_prefix = true;
                match prefix.kind() {
                    Prefix::Disk(_)
                    | Prefix::UNC(_, _)
                    | Prefix::VerbatimDisk(_)
                    | Prefix::VerbatimUNC(_, _) => {}
                    Prefix::DeviceNS(_) | Prefix::Verbatim(_) => {
                        return Err(PersistenceError::new(
                            "console_unsupported_platform_operation",
                            "Windows device namespace paths are not supported for console persistence",
                        ));
                    }
                }
            }
            Component::Normal(value) => validate_windows_normal_component(value)?,
            Component::ParentDir => {
                return Err(PersistenceError::path_escape(
                    "Mutable console path contains parent traversal",
                ));
            }
            Component::RootDir | Component::CurDir => {}
        }
    }
    if !saw_prefix || !path.is_absolute() {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "Windows console persistence requires an absolute disk or UNC path",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn validate_unresolved_platform_path(path: &Path) -> Result<(), PersistenceError> {
    use std::path::Prefix;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(_)
                | Prefix::UNC(_, _)
                | Prefix::VerbatimDisk(_)
                | Prefix::VerbatimUNC(_, _) => {}
                Prefix::DeviceNS(_) | Prefix::Verbatim(_) => {
                    return Err(PersistenceError::new(
                        "console_unsupported_platform_operation",
                        "Windows device namespace paths are not supported for console persistence",
                    ));
                }
            },
            Component::Normal(value) => validate_windows_normal_component(value)?,
            Component::RootDir | Component::CurDir | Component::ParentDir => {}
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn validate_unresolved_platform_path(_path: &Path) -> Result<(), PersistenceError> {
    Ok(())
}

#[cfg(windows)]
fn validate_windows_normal_component(value: &std::ffi::OsStr) -> Result<(), PersistenceError> {
    use std::os::windows::ffi::OsStrExt;

    if value.encode_wide().any(|unit| unit == b':' as u16) {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "NTFS alternate data stream paths are not supported for console persistence",
        ));
    }
    let value = value.to_string_lossy();
    if value.ends_with(['.', ' ']) || is_windows_reserved_name(&value) {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "Windows reserved names and trailing dot/space components are not supported for console persistence",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reserved_name(value: &str) -> bool {
    let basename = value.split('.').next().unwrap_or(value);
    let uppercase = basename.to_ascii_uppercase();
    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || uppercase.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || uppercase.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

#[cfg(not(windows))]
fn validate_platform_path(path: &Path) -> Result<(), PersistenceError> {
    if !path.is_absolute() {
        return Err(PersistenceError::new(
            "console_unsupported_platform_operation",
            "Console persistence requires an absolute path",
        ));
    }
    Ok(())
}

fn reject_existing_links(path: &Path) -> Result<(), PersistenceError> {
    let mut current = PathBuf::new();
    let mut missing = false;
    for component in path.components() {
        current.push(component.as_os_str());
        if missing || matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => reject_link_metadata(&current, &metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => missing = true,
            Err(error) => return Err(PersistenceError::io("checking path components", error)),
        }
    }
    Ok(())
}

fn ensure_secure_directory(path: &Path) -> Result<(), PersistenceError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                reject_link_metadata(&current, &metadata)?;
                if !metadata.is_dir() {
                    return Err(PersistenceError::corruption(format!(
                        "Console state component '{}' is not a directory",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(PersistenceError::io(
                            "creating console state directory",
                            error,
                        ));
                    }
                }
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|error| PersistenceError::io("checking created directory", error))?;
                reject_link_metadata(&current, &metadata)?;
                if !metadata.is_dir() {
                    return Err(PersistenceError::corruption(
                        "Created console state component is not a directory",
                    ));
                }
            }
            Err(error) => return Err(PersistenceError::io("checking console state path", error)),
        }
    }
    Ok(())
}

fn ensure_regular_file(path: &Path) -> Result<(), PersistenceError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    configure_lock_sharing(&mut options);
    match options.open(path) {
        Ok(file) => {
            file.sync_all()
                .map_err(|error| PersistenceError::io("syncing writer lock", error))?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(PersistenceError::io("creating writer lock", error)),
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PersistenceError::io("checking writer lock", error))?;
    reject_link_metadata(path, &metadata)?;
    if !metadata.is_file() {
        return Err(PersistenceError::corruption(
            "Gateway console writer lock is not a regular file",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn configure_lock_sharing(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
}

#[cfg(not(windows))]
fn configure_lock_sharing(_options: &mut OpenOptions) {}

fn reject_link_metadata(path: &Path, metadata: &fs::Metadata) -> Result<(), PersistenceError> {
    if metadata.file_type().is_symlink() || is_reparse_point(metadata) {
        return Err(PersistenceError::new(
            "console_journal_path_escape",
            format!(
                "Gateway console persistence rejects symlink or reparse-point component '{}'",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn validate_archive_relative_path(value: &str) -> Result<(), PersistenceError> {
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(PersistenceError::path_escape(
            "Transaction archive path must be relative to the console state root",
        ));
    }
    let mut components = path.components();
    match (components.next(), components.next(), components.next()) {
        (Some(Component::Normal(root)), Some(Component::Normal(_)), None)
            if root == "revisions" =>
        {
            Ok(())
        }
        _ => Err(PersistenceError::path_escape(
            "Transaction archive path must be revisions/<revision-id>",
        )),
    }
}

fn validate_safe_leaf(value: &str) -> Result<(), PersistenceError> {
    let path = Path::new(value);
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(PersistenceError::path_escape(
            "Same-directory backup path must be a single relative leaf",
        ));
    }
    #[cfg(windows)]
    if value.contains(':') {
        return Err(PersistenceError::path_escape(
            "Same-directory backup leaf must not name an NTFS alternate data stream",
        ));
    }
    Ok(())
}

fn validate_safe_id(kind: &str, value: &str) -> Result<(), PersistenceError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(PersistenceError::corruption(format!(
            "Invalid {kind} identifier"
        )));
    }
    Ok(())
}

fn validate_error_code(value: &str) -> Result<(), PersistenceError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        return Err(PersistenceError::corruption(
            "Transaction error code must be a bounded stable ASCII code",
        ));
    }
    Ok(())
}

fn validate_revision_id(value: &str) -> Result<(), PersistenceError> {
    let Some((sequence, digest)) = value
        .strip_prefix('r')
        .and_then(|value| value.split_once('-'))
    else {
        return Err(PersistenceError::corruption("Invalid revision identifier"));
    };
    let Ok(parsed) = sequence.parse::<u64>() else {
        return Err(PersistenceError::corruption("Invalid revision sequence"));
    };
    if sequence != parsed.to_string()
        || digest.len() != 12
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(PersistenceError::corruption("Invalid revision identifier"));
    }
    Ok(())
}

fn revision_sequence(value: &str) -> u64 {
    value
        .strip_prefix('r')
        .and_then(|value| value.split_once('-'))
        .and_then(|(sequence, _)| sequence.parse().ok())
        .unwrap_or(u64::MAX)
}

fn normalized_relative_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn read_json_limited<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, PersistenceError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PersistenceError::io("reading JSON metadata", error))?;
    reject_link_metadata(path, &metadata)?;
    if !metadata.is_file() || metadata.len() > MAX_METADATA_BYTES {
        return Err(PersistenceError::corruption(
            "Console persistence JSON is not a bounded regular file",
        ));
    }
    let bytes = read_regular_file(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| PersistenceError::corruption("Console persistence JSON is malformed"))
}

pub(crate) fn read_transaction_json(path: &Path) -> Result<TransactionRecord, PersistenceError> {
    let record: TransactionRecord = read_json_limited(path)?;
    record.validate()?;
    validate_transaction_record_path(path, &record)?;
    Ok(record)
}

fn validate_transaction_record_path(
    path: &Path,
    record: &TransactionRecord,
) -> Result<(), PersistenceError> {
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err(PersistenceError::corruption(
            "Transaction record path must use the .json extension",
        ));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            PersistenceError::corruption(
                "Transaction record path must have a valid UTF-8 file stem",
            )
        })?;
    if stem != record.tx_id() {
        return Err(PersistenceError::corruption(
            "Transaction record filename does not match its tx_id",
        ));
    }
    Ok(())
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, PersistenceError> {
    read_regular_file_io(path).map_err(|error| PersistenceError::io("reading durable file", error))
}

fn read_regular_file_io(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not a non-link regular file",
        ));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn create_new_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PersistenceError> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| PersistenceError::io("serializing durable JSON", error))?;
    bytes.push(b'\n');
    create_new_bytes(path, &bytes)
}

fn create_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), PersistenceError> {
    create_new_bytes_io(path, bytes)
        .map_err(|error| PersistenceError::io("creating durable file", error))
}

fn create_new_bytes_io(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    Ok(())
}

fn write_same_dir_temp(
    target: &Path,
    label: &str,
    bytes: &[u8],
) -> Result<PathBuf, PersistenceError> {
    let parent = target.parent().ok_or_else(|| {
        PersistenceError::new(
            "console_unsupported_platform_operation",
            "Atomic target has no parent directory",
        )
    })?;
    for _ in 0..8 {
        let path = parent.join(format!(".gateway-console-{}-{label}.tmp", Uuid::new_v4()));
        match create_new_bytes_io(&path, bytes) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(PersistenceError::io("writing same-directory temp", error)),
        }
    }
    Err(PersistenceError::io(
        "allocating same-directory temp",
        "exhausted unpredictable file names",
    ))
}

fn unique_sibling(target: &Path, label: &str) -> PathBuf {
    target
        .parent()
        .unwrap()
        .join(format!(".gateway-console-{}-{label}", Uuid::new_v4()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn is_lowercase_digest(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), PersistenceError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| PersistenceError::io("syncing parent directory", error))
}

#[cfg(windows)]
fn sync_directory(_path: &Path) -> Result<(), PersistenceError> {
    Ok(())
}
