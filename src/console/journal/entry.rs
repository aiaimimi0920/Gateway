//! Journal entry conversion and validation preserve immutable transaction metadata.

use super::{
    JournalEntry, PersistenceError, TransactionPhase, TransactionRecord, YamlReplaceReceipt,
};
use time::OffsetDateTime;

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

    pub(super) fn from_record(record: &TransactionRecord) -> Self {
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

    pub(super) fn validate(&self) -> Result<(), PersistenceError> {
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

    pub(super) fn matches_transaction(&self, record: &TransactionRecord) -> bool {
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
