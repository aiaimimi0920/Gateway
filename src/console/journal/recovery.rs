//! Local recovery validates artifacts before rollback or deferral to Redis reconciliation.

use super::validation::{validate_journal_order, validate_journal_transaction_cross_refs};
use super::{
    PersistenceError, RecoveryDisposition, RecoveryReport, RouteConfigPersistence,
    TransactionJournal, TransactionPhase, TransactionRecord, WriterLockGuard, YamlReplaceReceipt,
};
use std::{fs, io};
use time::OffsetDateTime;

impl TransactionJournal {
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

    pub(super) fn validate_scratch_files(
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
