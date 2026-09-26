//! Journal history and transaction records must agree on forward-only durable phases.

use super::{JournalEntry, PersistenceError, TransactionPhase, TransactionRecord};
use std::collections::BTreeMap;

pub(super) fn ensure_record_progression(
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

pub(super) fn validate_journal_order(entries: &[JournalEntry]) -> Result<(), PersistenceError> {
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

pub(super) fn validate_journal_transaction_cross_refs(
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
