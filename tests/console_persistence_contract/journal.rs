//! Journal framing, phase transitions and admission limits fail closed before publication.

use super::{sha256, transaction, TestDirectory};
use neuro_gateway::console::journal::TransactionJournal;
use neuro_gateway::console::persistence::{
    RouteConfigPersistence, TransactionPhase, TransactionRecord,
};
use std::fs;
use std::io::Write as _;
use time::OffsetDateTime;

#[test]
fn transaction_phase_machine_is_forward_only_and_aborts_before_redis_only() {
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let mut record = TransactionRecord::new_prepared(
        "tx-1",
        Some("r0-000000000000"),
        "r1-111111111111",
        "revisions/r1-111111111111",
        false,
        None,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        now,
    )
    .unwrap();

    record
        .transition(TransactionPhase::YamlReplaced, now, None)
        .unwrap();
    let error = record
        .transition(TransactionPhase::Prepared, now, None)
        .unwrap_err();
    assert_eq!(error.code(), "console_invalid_phase");

    record
        .transition(TransactionPhase::RedisActivated, now, None)
        .unwrap();
    let error = record
        .transition(TransactionPhase::Aborted, now, Some("redis_unknown"))
        .unwrap_err();
    assert_eq!(error.code(), "console_invalid_phase");
}

#[test]
fn durable_journal_accepts_the_complete_normal_phase_chain() {
    let temp = TestDirectory::new("normal-phase-chain");
    let routes = temp.path().join("routes.yaml");
    let old = b"providers: []\n";
    let new = b"aliases: {}\n";
    fs::write(&routes, old).unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let record = TransactionRecord::new_prepared(
        "tx-normal-chain",
        Some("r0-000000000000"),
        "r1-111111111111",
        "revisions/r1-111111111111",
        true,
        Some(&sha256(old)),
        sha256(new),
        now,
    )
    .unwrap();
    journal.persist(&record).unwrap();
    let receipt = persistence
        .replace_routes_yaml_for_transaction(&record, new)
        .unwrap();
    journal
        .record_yaml_replaced("tx-normal-chain", &receipt, now)
        .unwrap();
    journal
        .transition(
            "tx-normal-chain",
            TransactionPhase::RedisActivated,
            now,
            None,
        )
        .unwrap();
    journal
        .transition("tx-normal-chain", TransactionPhase::Committed, now, None)
        .unwrap();

    let entries = journal.load_entries().unwrap();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries.last().unwrap().phase(), TransactionPhase::Committed);
}

#[test]
fn generic_journal_transition_cannot_claim_yaml_replacement_without_a_receipt() {
    let temp = TestDirectory::new("fake-yaml-phase");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence);
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let record = transaction("tx-fake-yaml-phase", "r1-111111111111", now);
    journal.persist(&record).unwrap();

    let error = journal
        .transition(record.tx_id(), TransactionPhase::YamlReplaced, now, None)
        .unwrap_err();

    assert_eq!(error.code(), "console_invalid_phase");
    assert_eq!(
        journal.load_transaction(record.tx_id()).unwrap().phase(),
        TransactionPhase::Prepared
    );
}

#[test]
fn journal_rejects_revision_paths_outside_state_root() {
    let record = TransactionRecord::prepared("tx-1", "../outside", "r1-a", "r2-b");
    let error = TransactionJournal::validate_record(&record).unwrap_err();
    assert_eq!(error.code(), "console_journal_path_escape");
}

#[test]
fn journal_repairs_only_an_incomplete_final_line_before_append() {
    let temp = TestDirectory::new("journal-tail");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let first = transaction("tx-1", "r1-111111111111", now);
    let second = transaction("tx-2", "r2-222222222222", now);
    journal.persist(&first).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(persistence.journal_path())
        .unwrap()
        .write_all(b"{\"version\":1,\"txId\":")
        .unwrap();

    journal.persist(&second).unwrap();
    let entries = journal.load_entries().unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].tx_id(), "tx-1");
    assert_eq!(entries[1].tx_id(), "tx-2");
    assert!(fs::read(persistence.journal_path())
        .unwrap()
        .ends_with(b"\n"));
}

#[test]
fn journal_accepts_a_complete_final_record_without_lf_and_repairs_it_before_append() {
    let temp = TestDirectory::new("journal-no-final-lf");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let first = transaction("tx-no-lf-1", "r1-111111111111", now);
    let second = transaction("tx-no-lf-2", "r2-222222222222", now);
    journal.persist(&first).unwrap();
    let path = persistence.journal_path();
    let mut bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.pop(), Some(b'\n'));
    fs::write(&path, bytes).unwrap();

    assert_eq!(journal.load_entries().unwrap().len(), 1);
    journal.persist(&second).unwrap();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.windows(2).any(|window| window == b"}\n"));
    assert_eq!(journal.load_entries().unwrap().len(), 2);
}

#[test]
fn journal_rejects_a_cr_terminated_final_record_without_lf() {
    let temp = TestDirectory::new("journal-final-cr");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let record = transaction(
        "tx-final-cr",
        "r1-111111111111",
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    );
    journal.persist(&record).unwrap();
    let path = persistence.journal_path();
    let mut bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.pop(), Some(b'\n'));
    bytes.push(b'\r');
    fs::write(&path, bytes).unwrap();

    let error = journal.load_entries().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(persistence.is_read_only());
}

#[test]
fn transaction_error_codes_are_bounded_metadata_not_free_text() {
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let invalid_codes = [
        "contains secret".to_string(),
        "line\nbreak".to_string(),
        "x".repeat(129),
    ];
    for invalid in &invalid_codes {
        let mut record = TransactionRecord::new_prepared(
            "tx-error-code",
            Some("r0-000000000000"),
            "r1-111111111111",
            "revisions/r1-111111111111",
            false,
            None,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            now,
        )
        .unwrap();
        let error = record
            .transition(TransactionPhase::Aborted, now, Some(invalid))
            .unwrap_err();
        assert_eq!(error.code(), "console_corruption");
        assert_eq!(record.phase(), TransactionPhase::Prepared);
    }
}

#[test]
fn journal_rejects_malformed_newline_terminated_records() {
    let temp = TestDirectory::new("journal-corruption");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    fs::write(persistence.journal_path(), b"{not-json}\n").unwrap();
    let journal = TransactionJournal::new(persistence.clone());

    let error = journal.load_entries().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(persistence.is_read_only());
}

#[test]
fn journal_line_limit_is_enforced_before_unbounded_or_transaction_growth() {
    let temp = TestDirectory::new("journal-line-limit");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::with_limits(persistence.clone(), 10, 128).unwrap();
    let record = transaction(
        "tx-line-limit",
        "r1-111111111111",
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    );

    let error = journal.persist(&record).unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(!persistence
        .transaction_record_path(record.tx_id())
        .unwrap()
        .exists());
    assert!(!persistence.journal_path().exists());
}

#[test]
fn journal_record_limit_is_checked_before_writing_the_next_transaction_json() {
    let temp = TestDirectory::new("journal-record-limit");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let journal = TransactionJournal::with_limits(persistence.clone(), 1, 1024 * 1024).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let first = transaction("tx-record-limit-1", "r1-111111111111", now);
    let second = transaction("tx-record-limit-2", "r2-222222222222", now);
    journal.persist(&first).unwrap();

    let error = journal.persist(&second).unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(!persistence
        .transaction_record_path(second.tx_id())
        .unwrap()
        .exists());
    assert_eq!(journal.load_entries().unwrap().len(), 1);
}

#[test]
fn oversized_existing_journal_line_is_rejected_at_the_configured_bound() {
    let temp = TestDirectory::new("journal-existing-line-limit");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    fs::write(persistence.journal_path(), vec![b'x'; 129]).unwrap();
    let journal = TransactionJournal::with_limits(persistence.clone(), 10, 128).unwrap();

    let error = journal.load_entries().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert!(persistence.is_read_only());
}
