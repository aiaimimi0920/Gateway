use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::persistence::{
    read_transaction_json, PersistenceError, RouteConfigPersistence, WriterLockGuard,
    YamlReplaceReceipt,
};
pub use super::persistence::{TransactionPhase, TransactionRecord};

const MAX_JOURNAL_RECORDS: usize = 100_000;
const MAX_JOURNAL_LINE_BYTES: usize = 1024 * 1024;

pub type JournalError = PersistenceError;

pub(crate) trait JournalAppendBackend: fmt::Debug + Send + Sync {
    fn append_and_sync(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
}

#[derive(Debug, Default)]
struct FileJournalAppendBackend;

impl JournalAppendBackend for FileJournalAppendBackend {
    fn append_and_sync(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDisposition {
    Clean,
    RecoveredLocally,
    RequiresRedisRecovery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryReport {
    disposition: RecoveryDisposition,
    recovered_transactions: usize,
    redis_transactions: usize,
}

impl RecoveryReport {
    pub fn disposition(&self) -> RecoveryDisposition {
        self.disposition
    }

    pub fn recovered_transactions(&self) -> usize {
        self.recovered_transactions
    }

    pub fn redis_transactions(&self) -> usize {
        self.redis_transactions
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalEntry {
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
    timestamp: OffsetDateTime,
    abort_code: Option<String>,
    error_code: Option<String>,
}

impl JournalEntry {
    pub fn tx_id(&self) -> &str {
        &self.tx_id
    }

    pub fn phase(&self) -> TransactionPhase {
        self.phase
    }

    pub fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    pub fn archive_relative_path(&self) -> &str {
        &self.archive_relative_path
    }

    fn from_record(record: &TransactionRecord) -> Self {
        Self {
            version: 1,
            tx_id: record.tx_id().to_string(),
            phase: record.phase(),
            old_revision: record.old_revision().map(str::to_string),
            new_revision: record.new_revision().to_string(),
            archive_relative_path: record.archive_relative_path().to_string(),
            previous_yaml_present: record.previous_yaml_present(),
            previous_yaml_digest: record.previous_yaml_digest().map(str::to_string),
            new_yaml_digest: record.new_yaml_digest().to_string(),
            same_dir_backup_leaf: record.same_dir_backup_leaf().map(str::to_string),
            timestamp: record.updated_at(),
            abort_code: record.abort_code().map(str::to_string),
            error_code: record.error_code().map(str::to_string),
        }
    }

    fn to_record(&self) -> Result<TransactionRecord, PersistenceError> {
        let mut record = TransactionRecord::new_prepared(
            self.tx_id.clone(),
            self.old_revision.as_deref(),
            self.new_revision.clone(),
            self.archive_relative_path.clone(),
            self.previous_yaml_present,
            self.previous_yaml_digest.as_deref(),
            self.new_yaml_digest.clone(),
            self.timestamp,
        )?;
        record.set_prepared_backup_leaf(self.same_dir_backup_leaf.as_deref())?;
        if matches!(
            self.phase,
            TransactionPhase::YamlReplaced
                | TransactionPhase::RedisActivated
                | TransactionPhase::Committed
        ) {
            let receipt = YamlReplaceReceipt::from_record_parts(
                self.previous_yaml_present,
                self.previous_yaml_digest.as_deref(),
                &self.new_yaml_digest,
                self.same_dir_backup_leaf.as_deref(),
            );
            record.record_yaml_replaced(&receipt, self.timestamp)?;
        }
        if self.phase == TransactionPhase::RedisActivated {
            record.transition(TransactionPhase::RedisActivated, self.timestamp, None)?;
        } else if self.phase == TransactionPhase::Committed {
            record.transition(TransactionPhase::RedisActivated, self.timestamp, None)?;
            record.transition(TransactionPhase::Committed, self.timestamp, None)?;
        } else if self.phase == TransactionPhase::Aborted {
            record.transition(
                TransactionPhase::Aborted,
                self.timestamp,
                self.error_code.as_deref(),
            )?;
        }
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> Result<(), PersistenceError> {
        if self.version != 1 {
            return Err(PersistenceError::corruption(
                "Unsupported console journal entry version",
            ));
        }
        if self.phase == TransactionPhase::Aborted {
            if self.abort_code.is_none()
                || self
                    .error_code
                    .as_deref()
                    .is_some_and(|code| Some(code) != self.abort_code.as_deref())
            {
                return Err(PersistenceError::corruption(
                    "Journal aborted error and abort codes are inconsistent",
                ));
            }
        } else if self.abort_code.is_some() || self.error_code.is_some() {
            return Err(PersistenceError::corruption(
                "Non-aborted journal entry contains an abort or error code",
            ));
        }
        let record = self.to_record()?;
        if record.phase() != self.phase
            || record.tx_id() != self.tx_id
            || record.old_revision() != self.old_revision.as_deref()
            || record.new_revision() != self.new_revision
            || record.archive_relative_path() != self.archive_relative_path
            || record.previous_yaml_present() != self.previous_yaml_present
            || record.previous_yaml_digest() != self.previous_yaml_digest.as_deref()
            || record.new_yaml_digest() != self.new_yaml_digest
            || record.same_dir_backup_leaf() != self.same_dir_backup_leaf.as_deref()
            || record.updated_at() != self.timestamp
            || record.abort_code() != self.abort_code.as_deref()
            || record.error_code() != self.error_code.as_deref()
        {
            return Err(PersistenceError::corruption(
                "Journal entry does not match its transaction metadata",
            ));
        }
        Ok(())
    }

    fn matches_transaction(&self, record: &TransactionRecord) -> bool {
        self.tx_id == record.tx_id()
            && self.old_revision.as_deref() == record.old_revision()
            && self.new_revision == record.new_revision()
            && self.archive_relative_path == record.archive_relative_path()
            && self.previous_yaml_present == record.previous_yaml_present()
            && self.previous_yaml_digest.as_deref() == record.previous_yaml_digest()
            && self.new_yaml_digest == record.new_yaml_digest()
            && self.same_dir_backup_leaf.as_deref() == record.same_dir_backup_leaf()
    }
}

#[derive(Clone, Debug)]
pub struct TransactionJournal {
    persistence: RouteConfigPersistence,
    max_records: usize,
    max_line_bytes: usize,
    append_backend: Arc<dyn JournalAppendBackend>,
}

impl TransactionJournal {
    pub fn new(persistence: RouteConfigPersistence) -> Self {
        Self {
            persistence,
            max_records: MAX_JOURNAL_RECORDS,
            max_line_bytes: MAX_JOURNAL_LINE_BYTES,
            append_backend: Arc::new(FileJournalAppendBackend),
        }
    }

    pub fn with_limits(
        persistence: RouteConfigPersistence,
        max_records: usize,
        max_line_bytes: usize,
    ) -> Result<Self, PersistenceError> {
        if max_records == 0
            || max_records > MAX_JOURNAL_RECORDS
            || max_line_bytes < 128
            || max_line_bytes > MAX_JOURNAL_LINE_BYTES
        {
            return Err(PersistenceError::corruption(
                "Journal limits must stay within the supported bounded range",
            ));
        }
        Ok(Self {
            persistence,
            max_records,
            max_line_bytes,
            append_backend: Arc::new(FileJournalAppendBackend),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_append_backend_for_test(
        persistence: RouteConfigPersistence,
        append_backend: Arc<dyn JournalAppendBackend>,
    ) -> Self {
        Self {
            persistence,
            max_records: MAX_JOURNAL_RECORDS,
            max_line_bytes: MAX_JOURNAL_LINE_BYTES,
            append_backend,
        }
    }

    pub fn persistence(&self) -> &RouteConfigPersistence {
        &self.persistence
    }

    pub fn validate_record(record: &TransactionRecord) -> Result<(), PersistenceError> {
        record.validate()
    }

    pub fn persist(&self, record: &TransactionRecord) -> Result<(), PersistenceError> {
        self.persistence.ensure_writable()?;
        Self::validate_record(record)?;
        let guard = self.persistence.try_writer_lock()?;
        let result = self.persist_locked(&guard, record);
        if let Err(error) = &result {
            if matches!(
                error.code(),
                "console_corruption" | "console_journal_path_escape" | "console_recovery_required"
            ) {
                self.persistence.mark_degraded(error.code());
            }
        }
        result
    }

    pub fn append(&self, record: &TransactionRecord) -> Result<(), PersistenceError> {
        self.persist(record)
    }

    pub fn transition(
        &self,
        tx_id: &str,
        phase: TransactionPhase,
        timestamp: OffsetDateTime,
        error_code: Option<&str>,
    ) -> Result<TransactionRecord, PersistenceError> {
        self.persistence.ensure_writable()?;
        let guard = self.persistence.try_writer_lock()?;
        self.transition_locked(&guard, tx_id, phase, timestamp, error_code)
    }

    pub fn record_yaml_replaced(
        &self,
        tx_id: &str,
        receipt: &YamlReplaceReceipt,
        timestamp: OffsetDateTime,
    ) -> Result<TransactionRecord, PersistenceError> {
        self.persistence.ensure_writable()?;
        let guard = self.persistence.try_writer_lock()?;
        self.record_yaml_replaced_locked(&guard, tx_id, receipt, timestamp)
    }

    pub(crate) fn record_yaml_replaced_locked(
        &self,
        guard: &WriterLockGuard,
        tx_id: &str,
        receipt: &YamlReplaceReceipt,
        timestamp: OffsetDateTime,
    ) -> Result<TransactionRecord, PersistenceError> {
        self.persistence.ensure_guard(guard)?;
        self.persistence.ensure_writable()?;
        let path = self.persistence.transaction_path(tx_id)?;
        let mut record = read_transaction_json(&path)?;
        record.record_yaml_replaced(receipt, timestamp)?;
        self.persist_locked(guard, &record)?;
        Ok(record)
    }

    pub(crate) fn transition_locked(
        &self,
        guard: &WriterLockGuard,
        tx_id: &str,
        phase: TransactionPhase,
        timestamp: OffsetDateTime,
        error_code: Option<&str>,
    ) -> Result<TransactionRecord, PersistenceError> {
        self.persistence.ensure_guard(guard)?;
        self.persistence.ensure_writable()?;
        if phase == TransactionPhase::YamlReplaced {
            return Err(PersistenceError::invalid_phase(
                "YamlReplaced must be recorded with the durable replacement receipt",
            ));
        }
        let path = self.persistence.transaction_path(tx_id)?;
        let mut record = read_transaction_json(&path)?;
        record.transition(phase, timestamp, error_code)?;
        self.persist_locked(guard, &record)?;
        Ok(record)
    }

    pub fn load_entries(&self) -> Result<Vec<JournalEntry>, PersistenceError> {
        match self.load_entries_inner(false) {
            Ok(entries) => Ok(entries),
            Err(error) => {
                self.persistence.mark_degraded(error.code());
                Err(error)
            }
        }
    }

    pub fn load(&self) -> Result<Vec<JournalEntry>, PersistenceError> {
        self.load_entries()
    }

    pub fn load_transactions(&self) -> Result<Vec<TransactionRecord>, PersistenceError> {
        let mut paths = Vec::new();
        let entries = fs::read_dir(self.persistence.transactions_dir())
            .map_err(|error| PersistenceError::io("listing transaction records", error))?;
        for entry in entries {
            let entry = entry
                .map_err(|error| PersistenceError::io("reading transaction directory", error))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                return Err(PersistenceError::corruption(
                    "Transaction directory contains an unexpected file",
                ));
            }
            paths.push(path);
        }
        paths.sort();
        let mut records = Vec::with_capacity(paths.len());
        for path in paths {
            records.push(read_transaction_json(&path)?);
        }
        Ok(records)
    }

    pub fn load_transaction(&self, tx_id: &str) -> Result<TransactionRecord, PersistenceError> {
        let path = self.persistence.transaction_path(tx_id)?;
        read_transaction_json(&path)
    }

    pub fn recover_local(&self) -> Result<RecoveryReport, PersistenceError> {
        let guard = self.persistence.try_writer_lock()?;
        let result = self.recover_local_locked(&guard);
        if let Err(error) = &result {
            self.persistence
                .mark_degraded(if error.code() == "console_corruption" {
                    "console_corruption"
                } else {
                    "console_recovery_required"
                });
        }
        result
    }

    pub fn recover(&self) -> Result<RecoveryReport, PersistenceError> {
        self.recover_local()
    }

    pub(crate) fn inspect_locked(
        &self,
        guard: &WriterLockGuard,
    ) -> Result<Vec<TransactionRecord>, PersistenceError> {
        self.persistence.ensure_guard(guard)?;
        self.load_entries_inner(true)?;
        let entries = self.load_entries_inner(false)?;
        validate_journal_order(&entries)?;
        let mut records = self.load_transactions()?;
        records.sort_by(|left, right| {
            left.created_at()
                .cmp(&right.created_at())
                .then_with(|| left.tx_id().cmp(right.tx_id()))
        });
        validate_journal_transaction_cross_refs(&entries, &records)?;
        self.validate_scratch_files(&records)?;
        for record in &records {
            self.persistence.verify_transaction_archive(record)?;
        }
        Ok(records)
    }

    pub(crate) fn persist_locked(
        &self,
        guard: &WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<(), PersistenceError> {
        self.persistence.ensure_guard(guard)?;
        self.persistence.ensure_writable()?;
        Self::validate_record(record)?;
        let entries = self.load_entries_inner(true)?;
        validate_journal_order(&entries)?;
        if entries.len() >= self.max_records {
            return Err(PersistenceError::corruption(
                "Journal record count exceeds the configured limit",
            ));
        }
        let bytes = self.serialize_entry(record)?;
        let path = self.persistence.transaction_path(record.tx_id())?;
        if path.exists() {
            let current = read_transaction_json(&path)?;
            ensure_record_progression(&current, record)?;
        }
        self.persistence
            .atomic_write_json_locked(guard, &path, record)?;
        if let Err(error) = self.append_bytes_locked(guard, &bytes) {
            self.persistence.mark_degraded("console_recovery_required");
            return Err(PersistenceError::recovery_required(format!(
                "Transaction JSON is durable but journal append requires recovery: {}",
                error.code()
            )));
        }
        Ok(())
    }

    fn serialize_entry(&self, record: &TransactionRecord) -> Result<Vec<u8>, PersistenceError> {
        let entry = JournalEntry::from_record(record);
        entry.validate()?;
        let mut bytes = serde_json::to_vec(&entry)
            .map_err(|error| PersistenceError::io("serializing journal entry", error))?;
        bytes.push(b'\n');
        if bytes.len() > self.max_line_bytes {
            return Err(PersistenceError::corruption(
                "Serialized journal record exceeds the configured line limit",
            ));
        }
        Ok(bytes)
    }

    fn append_bytes_locked(
        &self,
        guard: &WriterLockGuard,
        bytes: &[u8],
    ) -> Result<(), PersistenceError> {
        self.persistence.ensure_guard(guard)?;
        let path = self.persistence.journal_path();
        self.persistence.validate_optional_managed_file(&path)?;
        self.append_backend
            .append_and_sync(&path, bytes)
            .map_err(|error| PersistenceError::io("syncing journal", error))?;
        Ok(())
    }

    fn load_entries_inner(&self, repair_tail: bool) -> Result<Vec<JournalEntry>, PersistenceError> {
        let (entries, tail_offset, file_len, missing_final_lf) = self.scan_journal()?;
        if repair_tail && tail_offset < file_len {
            let file = OpenOptions::new()
                .write(true)
                .open(self.persistence.journal_path())
                .map_err(|error| PersistenceError::io("opening journal for tail repair", error))?;
            file.set_len(tail_offset)
                .and_then(|_| file.sync_all())
                .map_err(|error| PersistenceError::io("truncating journal tail", error))?;
        } else if repair_tail && missing_final_lf {
            let mut file = OpenOptions::new()
                .append(true)
                .open(self.persistence.journal_path())
                .map_err(|error| PersistenceError::io("opening journal for LF repair", error))?;
            file.write_all(b"\n")
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_all())
                .map_err(|error| PersistenceError::io("repairing journal LF terminator", error))?;
        }
        Ok(entries)
    }

    fn scan_journal(&self) -> Result<(Vec<JournalEntry>, u64, u64, bool), PersistenceError> {
        let path = self.persistence.journal_path();
        self.persistence.validate_optional_managed_file(&path)?;
        let file = match OpenOptions::new().read(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok((Vec::new(), 0, 0, false));
            }
            Err(error) => return Err(PersistenceError::io("opening journal", error)),
        };
        let file_len = file
            .metadata()
            .map_err(|error| PersistenceError::io("reading journal length", error))?
            .len();
        let mut reader = BufReader::new(file);
        let mut offset = 0u64;
        let mut last_good_offset = 0u64;
        let mut entries = Vec::new();
        let mut missing_final_lf = false;
        loop {
            let Some(mut line) = self.read_bounded_line(&mut reader)? else {
                break;
            };
            offset += line.len() as u64;
            let terminated = line.last() == Some(&b'\n');
            if terminated {
                line.pop();
                if line.last() == Some(&b'\r') {
                    return Err(PersistenceError::corruption(
                        "Journal records must use LF-only line endings",
                    ));
                }
                let entry: JournalEntry = serde_json::from_slice(&line).map_err(|_| {
                    PersistenceError::corruption(
                        "Malformed newline-terminated console journal record",
                    )
                })?;
                entry.validate()?;
                entries.push(entry);
                if entries.len() > self.max_records {
                    return Err(PersistenceError::corruption(
                        "Journal record count exceeds the configured limit",
                    ));
                }
                last_good_offset = offset;
            } else {
                if line.last() == Some(&b'\r') {
                    return Err(PersistenceError::corruption(
                        "Journal records must use LF-only line endings",
                    ));
                }
                match serde_json::from_slice::<JournalEntry>(&line) {
                    Ok(entry) => {
                        entry.validate()?;
                        entries.push(entry);
                        if entries.len() > self.max_records {
                            return Err(PersistenceError::corruption(
                                "Journal record count exceeds the configured limit",
                            ));
                        }
                        last_good_offset = offset;
                        missing_final_lf = true;
                    }
                    Err(error) if error.is_eof() => {
                        break;
                    }
                    Err(_) => {
                        return Err(PersistenceError::corruption(
                            "Malformed incomplete console journal tail",
                        ));
                    }
                }
            }
        }
        Ok((entries, last_good_offset, file_len, missing_final_lf))
    }

    fn read_bounded_line<R: BufRead>(
        &self,
        reader: &mut R,
    ) -> Result<Option<Vec<u8>>, PersistenceError> {
        let mut line = Vec::with_capacity(self.max_line_bytes.min(8 * 1024));
        loop {
            let available = reader
                .fill_buf()
                .map_err(|error| PersistenceError::io("reading journal", error))?;
            if available.is_empty() {
                return Ok((!line.is_empty()).then_some(line));
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let take = newline.map_or(available.len(), |index| index + 1);
            if line.len().saturating_add(take) > self.max_line_bytes {
                return Err(PersistenceError::corruption(
                    "Journal record exceeds the configured line limit",
                ));
            }
            line.extend_from_slice(&available[..take]);
            reader.consume(take);
            if newline.is_some() {
                return Ok(Some(line));
            }
        }
    }

    pub(crate) fn recover_local_locked(
        &self,
        guard: &WriterLockGuard,
    ) -> Result<RecoveryReport, PersistenceError> {
        self.persistence.ensure_guard(guard)?;
        self.load_entries_inner(true)?;
        let entries = self.load_entries_inner(false)?;
        validate_journal_order(&entries)?;
        let mut records = self.load_transactions()?;
        records.sort_by(|left, right| left.tx_id().cmp(right.tx_id()));
        validate_journal_transaction_cross_refs(&entries, &records)?;
        self.validate_scratch_files(&records)?;
        for record in &records {
            self.persistence.verify_transaction_archive(record)?;
        }

        let mut recovered = 0usize;
        let mut redis = 0usize;
        for mut record in records {
            match record.phase() {
                TransactionPhase::Prepared => {
                    if prepared_state_is_unchanged(&self.persistence, &record)? {
                        record.transition(
                            TransactionPhase::Aborted,
                            OffsetDateTime::now_utc(),
                            Some("prepared_without_yaml_replace"),
                        )?;
                        self.persist_locked(guard, &record)?;
                        recovered += 1;
                    } else if prepared_state_matches_new(&self.persistence, &record)? {
                        let receipt = YamlReplaceReceipt::from_record_parts(
                            record.previous_yaml_present(),
                            record.previous_yaml_digest(),
                            record.new_yaml_digest(),
                            record.same_dir_backup_leaf(),
                        );
                        self.persistence
                            .restore_routes_yaml_locked(guard, &receipt)?;
                        record.transition(
                            TransactionPhase::Aborted,
                            OffsetDateTime::now_utc(),
                            Some("yaml_replaced_before_phase_record"),
                        )?;
                        self.persist_locked(guard, &record)?;
                        recovered += 1;
                    } else {
                        return Err(PersistenceError::recovery_required(format!(
                            "Prepared transaction '{}' has ambiguous YAML state",
                            record.tx_id()
                        )));
                    }
                }
                TransactionPhase::YamlReplaced => {
                    let receipt = YamlReplaceReceipt::from_record_parts(
                        record.previous_yaml_present(),
                        record.previous_yaml_digest(),
                        record.new_yaml_digest(),
                        record.same_dir_backup_leaf(),
                    );
                    self.persistence
                        .restore_routes_yaml_locked(guard, &receipt)?;
                    record.transition(
                        TransactionPhase::Aborted,
                        OffsetDateTime::now_utc(),
                        Some("yaml_replaced_without_redis_activation"),
                    )?;
                    self.persist_locked(guard, &record)?;
                    recovered += 1;
                }
                TransactionPhase::RedisActivated => {
                    redis += 1;
                }
                TransactionPhase::Committed | TransactionPhase::Aborted => {}
            }
        }

        let disposition = if redis > 0 {
            RecoveryDisposition::RequiresRedisRecovery
        } else if recovered > 0 {
            RecoveryDisposition::RecoveredLocally
        } else {
            RecoveryDisposition::Clean
        };
        Ok(RecoveryReport {
            disposition,
            recovered_transactions: recovered,
            redis_transactions: redis,
        })
    }

    fn validate_scratch_files(
        &self,
        records: &[TransactionRecord],
    ) -> Result<(), PersistenceError> {
        let mut referenced = std::collections::BTreeSet::new();
        for record in records {
            if let Some(leaf) = record.same_dir_backup_leaf() {
                referenced.insert(leaf.to_string());
            }
        }
        let Some(parent) = self.persistence.routes_path().parent() else {
            return Ok(());
        };
        let entries = fs::read_dir(parent)
            .map_err(|error| PersistenceError::recovery_required(error.to_string()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| PersistenceError::recovery_required(error.to_string()))?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !name.starts_with(".gateway-console-") {
                continue;
            }
            let scratch = name.ends_with(".tmp")
                || name.ends_with("-bak")
                || name.ends_with("-rollback-bak")
                || name.ends_with("-json-bak");
            if scratch && !referenced.contains(name) {
                return Err(PersistenceError::recovery_required(format!(
                    "Unreferenced console persistence scratch file '{}' requires operator recovery",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

fn ensure_record_progression(
    current: &TransactionRecord,
    next: &TransactionRecord,
) -> Result<(), PersistenceError> {
    if current.tx_id() != next.tx_id() {
        return Err(PersistenceError::corruption(
            "Transaction record identity changed during update",
        ));
    }
    if current.old_revision() != next.old_revision()
        || current.new_revision() != next.new_revision()
        || current.archive_relative_path() != next.archive_relative_path()
        || current.previous_yaml_present() != next.previous_yaml_present()
        || current.previous_yaml_digest() != next.previous_yaml_digest()
        || current.new_yaml_digest() != next.new_yaml_digest()
        || current.same_dir_backup_leaf() != next.same_dir_backup_leaf()
        || current.created_at() != next.created_at()
        || next.updated_at() < current.updated_at()
    {
        return Err(PersistenceError::corruption(
            "Immutable transaction fields changed during phase update",
        ));
    }
    if current == next {
        return Ok(());
    }
    let allowed = matches!(
        (current.phase(), next.phase()),
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
            "Invalid persisted transaction update {:?} -> {:?}",
            current.phase(),
            next.phase()
        )));
    }
    Ok(())
}

fn phase_rank(phase: TransactionPhase) -> u8 {
    match phase {
        TransactionPhase::Prepared => 0,
        TransactionPhase::YamlReplaced => 1,
        TransactionPhase::RedisActivated => 2,
        TransactionPhase::Committed => 3,
        TransactionPhase::Aborted => 4,
    }
}

fn validate_journal_order(entries: &[JournalEntry]) -> Result<(), PersistenceError> {
    let mut phases = BTreeMap::<String, TransactionPhase>::new();
    for entry in entries {
        let Some(previous) = phases.get(entry.tx_id()).copied() else {
            phases.insert(entry.tx_id().to_string(), entry.phase());
            continue;
        };
        if previous == entry.phase() {
            continue;
        }
        let valid = match (previous, entry.phase()) {
            (TransactionPhase::Prepared, TransactionPhase::YamlReplaced)
            | (TransactionPhase::Prepared, TransactionPhase::Aborted)
            | (TransactionPhase::YamlReplaced, TransactionPhase::RedisActivated)
            | (TransactionPhase::RedisActivated, TransactionPhase::Committed)
            | (TransactionPhase::YamlReplaced, TransactionPhase::Aborted) => true,
            _ => false,
        };
        if !valid || phase_rank(entry.phase()) < phase_rank(previous) {
            return Err(PersistenceError::corruption(
                "Console journal contains a phase regression",
            ));
        }
        phases.insert(entry.tx_id().to_string(), entry.phase());
    }
    Ok(())
}

fn validate_journal_transaction_cross_refs(
    entries: &[JournalEntry],
    records: &[TransactionRecord],
) -> Result<(), PersistenceError> {
    let records = records
        .iter()
        .map(|record| (record.tx_id(), record))
        .collect::<BTreeMap<_, _>>();
    let mut latest = BTreeMap::<String, TransactionPhase>::new();
    for entry in entries {
        let record = records.get(entry.tx_id()).ok_or_else(|| {
            PersistenceError::corruption("Console journal references a missing transaction record")
        })?;
        if !entry.matches_transaction(record)
            || !journal_phase_can_appear_in_history(entry.phase(), record.phase())
        {
            return Err(PersistenceError::corruption(
                "Console journal and transaction record disagree",
            ));
        }
        latest.insert(entry.tx_id().to_string(), entry.phase());
    }
    for record in records.values() {
        let latest_phase = latest.get(record.tx_id()).copied().ok_or_else(|| {
            PersistenceError::recovery_required(
                "Console transaction record is missing its durable journal entry",
            )
        })?;
        if !latest_journal_phase_can_reach_transaction(latest_phase, record.phase()) {
            return Err(PersistenceError::recovery_required(
                "Console transaction record is ahead of its durable journal state",
            ));
        }
    }
    Ok(())
}

fn journal_phase_can_appear_in_history(
    journal: TransactionPhase,
    transaction: TransactionPhase,
) -> bool {
    journal == transaction
        || matches!(
            (journal, transaction),
            (TransactionPhase::Prepared, TransactionPhase::YamlReplaced)
                | (TransactionPhase::Prepared, TransactionPhase::RedisActivated)
                | (TransactionPhase::Prepared, TransactionPhase::Committed)
                | (TransactionPhase::Prepared, TransactionPhase::Aborted)
                | (
                    TransactionPhase::YamlReplaced,
                    TransactionPhase::RedisActivated
                )
                | (TransactionPhase::YamlReplaced, TransactionPhase::Committed)
                | (TransactionPhase::YamlReplaced, TransactionPhase::Aborted)
                | (
                    TransactionPhase::RedisActivated,
                    TransactionPhase::Committed
                )
        )
}

fn latest_journal_phase_can_reach_transaction(
    latest_journal: TransactionPhase,
    transaction: TransactionPhase,
) -> bool {
    latest_journal == transaction
        || matches!(
            (latest_journal, transaction),
            (TransactionPhase::Prepared, TransactionPhase::YamlReplaced)
                | (TransactionPhase::Prepared, TransactionPhase::Aborted)
                | (
                    TransactionPhase::YamlReplaced,
                    TransactionPhase::RedisActivated
                )
                | (TransactionPhase::YamlReplaced, TransactionPhase::Aborted)
                | (
                    TransactionPhase::RedisActivated,
                    TransactionPhase::Committed
                )
        )
}

fn prepared_state_is_unchanged(
    persistence: &RouteConfigPersistence,
    record: &TransactionRecord,
) -> Result<bool, PersistenceError> {
    let current = read_routes_bytes_for_recovery(persistence)?;
    Ok(match current {
        Some(bytes) => {
            record.previous_yaml_present()
                && record
                    .previous_yaml_digest()
                    .is_some_and(|digest| digest == sha256_hex(&bytes))
        }
        None => !record.previous_yaml_present(),
    })
}

fn prepared_state_matches_new(
    persistence: &RouteConfigPersistence,
    record: &TransactionRecord,
) -> Result<bool, PersistenceError> {
    Ok(read_routes_bytes_for_recovery(persistence)?
        .is_some_and(|bytes| sha256_hex(&bytes) == record.new_yaml_digest()))
}

fn read_routes_bytes_for_recovery(
    persistence: &RouteConfigPersistence,
) -> Result<Option<Vec<u8>>, PersistenceError> {
    match fs::symlink_metadata(persistence.routes_path()) {
        Ok(_) => {
            persistence
                .validate_optional_managed_file(persistence.routes_path())
                .map_err(|error| PersistenceError::recovery_required(error.to_string()))?;
            fs::read(persistence.routes_path())
                .map(Some)
                .map_err(|error| PersistenceError::recovery_required(error.to_string()))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PersistenceError::recovery_required(error.to_string())),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
