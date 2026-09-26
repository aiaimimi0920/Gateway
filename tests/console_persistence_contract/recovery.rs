//! Recovery verifies durable artifacts before rollback or deferral to Redis reconciliation.

use super::{sha256, transaction, TestDirectory};
use neuro_gateway::console::document::canonicalize_route_document;
use neuro_gateway::console::journal::{RecoveryDisposition, TransactionJournal};
use neuro_gateway::console::persistence::{
    RouteConfigPersistence, TransactionPhase, TransactionRecord,
};
use neuro_gateway::console::revision::{RevisionActor, RevisionMetadata};
use neuro_gateway::routing::config::RouteConfigYaml;
use std::fs;
use time::OffsetDateTime;

fn transaction_from_receipt(
    tx_id: &str,
    revision: &str,
    now: OffsetDateTime,
    receipt: &neuro_gateway::console::persistence::YamlReplaceReceipt,
) -> TransactionRecord {
    let mut record = TransactionRecord::new_prepared_with_backup_leaf(
        tx_id,
        Some("r0-000000000000"),
        revision,
        format!("revisions/{revision}"),
        receipt.previous_yaml_present(),
        receipt.previous_yaml_digest(),
        receipt.new_yaml_digest(),
        receipt.backup_leaf(),
        now,
    )
    .unwrap();
    record.record_yaml_replaced(receipt, now).unwrap();
    record
}

fn archive_candidate(
    persistence: &RouteConfigPersistence,
    now: OffsetDateTime,
) -> (String, Vec<u8>) {
    let document: RouteConfigYaml = serde_yaml::from_str(
        "providers: []\nmodel_routes: []\naliases:\n  managed: managed-model\n",
    )
    .unwrap();
    let canonical = canonicalize_route_document(&document).unwrap();
    let metadata = RevisionMetadata::from_canonical(
        1,
        Some("r0-000000000000".to_string()),
        RevisionActor::ManagementToken,
        now,
        None,
        &canonical,
    );
    persistence.archive_revision(&metadata, &canonical).unwrap();
    (
        metadata.id().to_string(),
        canonical.canonical_yaml().to_vec(),
    )
}

#[test]
fn prepared_transaction_with_new_yaml_digest_recovers_using_preallocated_backup() {
    let temp = TestDirectory::new("prepared-crash-gap");
    let routes = temp.path().join("routes.yaml");
    let old = b"providers: []\n";
    fs::write(&routes, old).unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let (revision, new) = archive_candidate(&persistence, now);
    let mut record = TransactionRecord::new_prepared(
        "tx-prepared-gap",
        Some("r0-000000000000"),
        &revision,
        format!("revisions/{revision}"),
        true,
        Some(&sha256(old)),
        sha256(&new),
        now,
    )
    .unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    journal.persist(&record).unwrap();
    let receipt = persistence
        .replace_routes_yaml_for_transaction(&record, &new)
        .unwrap();
    assert_eq!(receipt.backup_leaf(), record.same_dir_backup_leaf());

    let report = journal.recover_local().unwrap();
    record = journal.load_transaction("tx-prepared-gap").unwrap();

    assert_eq!(report.disposition(), RecoveryDisposition::RecoveredLocally);
    assert_eq!(record.phase(), TransactionPhase::Aborted);
    assert_eq!(fs::read(&routes).unwrap(), old);
}

#[test]
fn orphan_scratch_file_forces_explicit_read_only_recovery() {
    let temp = TestDirectory::new("orphan-scratch");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    fs::write(temp.path().join(".gateway-console-orphan.tmp"), b"partial").unwrap();
    let journal = TransactionJournal::new(persistence.clone());

    let error = journal.recover_local().unwrap_err();

    assert_eq!(error.code(), "console_recovery_required");
    assert!(persistence.is_read_only());
    assert!(!routes.exists());
}

#[test]
fn recovery_rejects_tampered_revision_archive() {
    let temp = TestDirectory::new("archive-recovery-tamper");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let document: RouteConfigYaml =
        serde_yaml::from_str("providers: []\nmodel_routes: []\naliases: {}\n").unwrap();
    let canonical = canonicalize_route_document(&document).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let metadata = RevisionMetadata::from_canonical(
        1,
        Some("r0-000000000000".to_string()),
        RevisionActor::ManagementToken,
        now,
        None,
        &canonical,
    );
    let archive = persistence.archive_revision(&metadata, &canonical).unwrap();
    let record = TransactionRecord::new_prepared(
        "tx-archive-tamper",
        Some("r0-000000000000"),
        metadata.id(),
        format!("revisions/{}", metadata.id()),
        false,
        None,
        canonical.yaml_digest(),
        now,
    )
    .unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    journal.persist(&record).unwrap();
    fs::write(archive.path().join("routes.yaml"), b"tampered\n").unwrap();

    let error = journal.recover_local().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(persistence.is_read_only());
}

#[test]
fn recovery_rejects_a_missing_referenced_revision_archive() {
    let temp = TestDirectory::new("archive-recovery-missing");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    journal
        .persist(&transaction(
            "tx-archive-missing",
            "r1-111111111111",
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        ))
        .unwrap();

    let error = journal.recover_local().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(persistence.is_read_only());
}

#[test]
fn recovery_rejects_a_journal_entry_without_its_transaction_json() {
    let temp = TestDirectory::new("journal-ghost-transaction");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let record = transaction(
        "tx-journal-ghost",
        "r1-111111111111",
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    );
    journal.persist(&record).unwrap();
    fs::remove_file(persistence.transaction_record_path(record.tx_id()).unwrap()).unwrap();

    let error = journal.recover_local().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(persistence.is_read_only());
}

#[test]
fn recovery_rejects_a_transaction_json_without_its_journal_entry() {
    let temp = TestDirectory::new("transaction-without-journal");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let record = transaction(
        "tx-without-journal",
        "r1-111111111111",
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    );
    journal.persist(&record).unwrap();
    fs::remove_file(persistence.journal_path()).unwrap();

    let error = journal.recover_local().unwrap_err();

    assert_eq!(error.code(), "console_recovery_required");
    assert!(persistence.is_read_only());
}

#[test]
fn recovery_rejects_a_transaction_record_that_jumps_ahead_of_its_latest_journal_phase() {
    let temp = TestDirectory::new("transaction-journal-phase-gap");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let record = transaction(
        "tx-phase-gap",
        "r1-111111111111",
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    );
    journal.persist(&record).unwrap();
    let path = persistence.transaction_record_path(record.tx_id()).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    json["phase"] = serde_json::Value::String("committed".to_string());
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string(&json).unwrap()),
    )
    .unwrap();

    let error = journal.recover_local().unwrap_err();

    assert_eq!(error.code(), "console_recovery_required");
    assert!(persistence.is_read_only());
}

#[test]
fn recovery_rejects_a_transaction_record_filename_mismatch() {
    let temp = TestDirectory::new("transaction-filename-mismatch");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let record = transaction(
        "tx-filename-match",
        "r1-111111111111",
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    );
    let original_path = persistence.transaction_record_path(record.tx_id()).unwrap();
    let mismatched_path = persistence
        .transactions_dir()
        .join("tx-filename-other.json");
    journal.persist(&record).unwrap();
    fs::rename(&original_path, &mismatched_path).unwrap();

    let error = journal.recover_local().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(persistence.is_read_only());
}

#[test]
fn yaml_replaced_transaction_recovers_the_old_yaml_and_aborts() {
    let temp = TestDirectory::new("recovery-yaml-replaced");
    let routes = temp.path().join("routes.yaml");
    fs::write(&routes, b"providers: []\n").unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let (revision, new) = archive_candidate(&persistence, now);
    let receipt = persistence.replace_routes_yaml(&new).unwrap();
    let record = transaction_from_receipt("tx-recover", &revision, now, &receipt);
    let journal = TransactionJournal::new(persistence.clone());
    journal.persist(&record).unwrap();

    let report = journal.recover_local().unwrap();
    let recovered = journal.load_transaction("tx-recover").unwrap();

    assert_eq!(report.disposition(), RecoveryDisposition::RecoveredLocally);
    assert_eq!(fs::read(&routes).unwrap(), b"providers: []\n");
    assert_eq!(recovered.phase(), TransactionPhase::Aborted);
    let replay = journal.recover_local().unwrap();
    assert_eq!(replay.disposition(), RecoveryDisposition::Clean);
}

#[test]
fn redis_activated_transaction_is_left_for_task_five() {
    let temp = TestDirectory::new("recovery-redis");
    let routes = temp.path().join("routes.yaml");
    fs::write(&routes, b"providers: []\n").unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let (revision, new) = archive_candidate(&persistence, now);
    let receipt = persistence.replace_routes_yaml(&new).unwrap();
    let mut record = transaction_from_receipt("tx-redis", &revision, now, &receipt);
    record
        .transition(TransactionPhase::RedisActivated, now, None)
        .unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    journal.persist(&record).unwrap();

    let report = journal.recover_local().unwrap();
    let recovered = journal.load_transaction("tx-redis").unwrap();

    assert_eq!(
        report.disposition(),
        RecoveryDisposition::RequiresRedisRecovery
    );
    assert_eq!(recovered.phase(), TransactionPhase::RedisActivated);
    assert_eq!(fs::read(&routes).unwrap(), new);
    assert!(!persistence.is_read_only());
}
