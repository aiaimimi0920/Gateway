//! Public journal contracts and single-writer persistence orchestration.

mod entry;
mod recovery;
mod storage;
mod validation;

use storage::FileJournalAppendBackend;
use validation::{
    ensure_record_progression, validate_journal_order, validate_journal_transaction_cross_refs,
};

use std::fmt;
use std::fs;
use std::io;
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
}
