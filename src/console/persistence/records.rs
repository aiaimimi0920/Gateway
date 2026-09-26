//! Transaction record constructors, receipts and forward-only phase invariants share one owner.

use super::validation::{
    is_lowercase_digest, normalized_relative_string, revision_sequence,
    validate_archive_relative_path, validate_error_code, validate_revision_id, validate_safe_id,
    validate_safe_leaf,
};
use super::{
    PersistenceError, TransactionPhase, TransactionRecord, YamlReplaceReceipt, STATE_VERSION,
};
use std::path::Path;
use time::OffsetDateTime;
use uuid::Uuid;

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
