use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use neuro_gateway::console::document::canonicalize_route_document;
use neuro_gateway::console::journal::{RecoveryDisposition, TransactionJournal};
use neuro_gateway::console::persistence::{
    AtomicReplaceBackend, FirstSavePresence, RouteConfigPersistence, TransactionPhase,
    TransactionRecord,
};
use neuro_gateway::console::revision::{RevisionActor, RevisionMetadata};
use neuro_gateway::routing::config::RouteConfigYaml;
use time::OffsetDateTime;

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gateway-console-persistence-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn state_layout_is_namespaced_and_does_not_bootstrap_task_six_files() {
    let temp = TestDirectory::new("layout");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();

    assert_eq!(persistence.state_root(), temp.path().join("console"));
    assert!(persistence.revisions_dir().is_dir());
    assert!(persistence.transactions_dir().is_dir());
    assert!(persistence.backups_dir().is_dir());
    assert!(persistence.writer_lock_path().is_file());
    assert!(!persistence.journal_path().exists());
    assert!(!persistence.state_root().join("admin.json").exists());
    assert!(!persistence.state_root().join("events.ndjson").exists());
    assert!(!persistence.state_root().join("auth.json").exists());
}

#[test]
fn first_console_save_preserves_original_bytes_exactly_and_is_idempotent() {
    let temp = TestDirectory::new("first-save-present");
    let routes = temp.path().join("routes.yaml");
    let original = b"\xef\xbb\xbf# original comment\r\nproviders: []\r\n";
    fs::write(&routes, original).unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();

    let first = persistence.ensure_first_save_backup().unwrap();
    let replay = persistence.ensure_first_save_backup().unwrap();

    assert_eq!(first.presence(), FirstSavePresence::Present);
    assert_eq!(replay, first);
    assert_eq!(
        fs::read(persistence.first_save_backup_path()).unwrap(),
        original
    );
}

#[test]
fn absent_first_save_state_survives_the_first_managed_yaml_write() {
    let temp = TestDirectory::new("first-save-absent");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();

    let first = persistence.ensure_first_save_backup().unwrap();
    assert_eq!(first.presence(), FirstSavePresence::Absent);
    assert!(!persistence.first_save_backup_path().exists());

    persistence
        .replace_routes_yaml(b"aliases: {}\nmodel_routes: []\nproviders: []\n")
        .unwrap();
    drop(persistence);

    let restarted = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let replay = restarted.ensure_first_save_backup().unwrap();
    assert_eq!(replay.presence(), FirstSavePresence::Absent);
    assert!(!restarted.first_save_backup_path().exists());
}

#[test]
fn corrupt_first_save_backup_is_never_overwritten() {
    let temp = TestDirectory::new("first-save-corruption");
    let routes = temp.path().join("routes.yaml");
    fs::write(&routes, b"# original\r\nproviders: []\r\n").unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    persistence.ensure_first_save_backup().unwrap();
    fs::write(persistence.first_save_backup_path(), b"tampered").unwrap();

    let error = persistence.ensure_first_save_backup().unwrap_err();

    assert_eq!(error.code(), "console_corruption");
    assert_eq!(
        fs::read(persistence.first_save_backup_path()).unwrap(),
        b"tampered"
    );
}

#[test]
fn canonical_yaml_rejects_bom_and_crlf_before_touching_the_target() {
    let temp = TestDirectory::new("canonical-yaml");
    let routes = temp.path().join("routes.yaml");
    fs::write(&routes, b"providers: []\n").unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();

    for invalid in [
        b"\xef\xbb\xbfproviders: []\n".as_slice(),
        b"providers: []\r\n".as_slice(),
        b"providers: [\xff]\n".as_slice(),
    ] {
        let error = persistence.replace_routes_yaml(invalid).unwrap_err();
        assert_eq!(error.code(), "console_persistence_invalid_yaml");
        assert_eq!(fs::read(&routes).unwrap(), b"providers: []\n");
    }
}

#[test]
fn replacing_and_restoring_existing_yaml_uses_verified_same_directory_backup() {
    let temp = TestDirectory::new("replace-present");
    let routes = temp.path().join("routes.yaml");
    let old = b"# legacy formatting\r\nproviders: []\r\n";
    let new = b"aliases: {}\nmodel_routes: []\nproviders: []\n";
    fs::write(&routes, old).unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();

    let receipt = persistence.replace_routes_yaml(new).unwrap();
    assert!(receipt.previous_yaml_present());
    assert_eq!(fs::read(&routes).unwrap(), new);
    let backup = routes
        .parent()
        .unwrap()
        .join(receipt.backup_leaf().unwrap());
    assert_eq!(backup.parent(), routes.parent());
    assert_eq!(fs::read(&backup).unwrap(), old);

    persistence.restore_routes_yaml(&receipt).unwrap();
    assert_eq!(fs::read(&routes).unwrap(), old);
}

#[test]
fn restoring_an_absent_yaml_requires_the_new_digest_to_match() {
    let temp = TestDirectory::new("replace-absent");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let receipt = persistence.replace_routes_yaml(b"providers: []\n").unwrap();
    assert!(!receipt.previous_yaml_present());
    fs::write(&routes, b"operator replacement\n").unwrap();

    let error = persistence.restore_routes_yaml(&receipt).unwrap_err();

    assert_eq!(error.code(), "console_recovery_required");
    assert_eq!(fs::read(&routes).unwrap(), b"operator replacement\n");
}

#[test]
fn immutable_revision_archive_is_idempotent_and_detects_tampering() {
    let temp = TestDirectory::new("archive");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let document: RouteConfigYaml =
        serde_yaml::from_str("providers: []\nmodel_routes: []\naliases: {}\n").unwrap();
    let canonical = canonicalize_route_document(&document).unwrap();
    let metadata = RevisionMetadata::from_canonical(
        1,
        Some("r0-000000000000".to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        Some("archive fixture".to_string()),
        &canonical,
    );

    let first = persistence.archive_revision(&metadata, &canonical).unwrap();
    let replay = persistence.archive_revision(&metadata, &canonical).unwrap();
    assert_eq!(replay, first);
    assert_eq!(
        fs::read(first.path().join("document.json")).unwrap(),
        canonical.canonical_json()
    );
    assert_eq!(
        fs::read(first.path().join("routes.yaml")).unwrap(),
        canonical.canonical_yaml()
    );

    fs::write(first.path().join("routes.yaml"), b"tampered\n").unwrap();
    let error = persistence
        .archive_revision(&metadata, &canonical)
        .unwrap_err();
    assert_eq!(error.code(), "console_corruption");
}

#[test]
fn immutable_revision_replay_rejects_unexpected_archive_entries() {
    let temp = TestDirectory::new("archive-extra-entry");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let document: RouteConfigYaml =
        serde_yaml::from_str("providers: []\nmodel_routes: []\naliases: {}\n").unwrap();
    let canonical = canonicalize_route_document(&document).unwrap();
    let metadata = RevisionMetadata::from_canonical(
        1,
        Some("r0-000000000000".to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        None,
        &canonical,
    );
    let archive = persistence.archive_revision(&metadata, &canonical).unwrap();
    fs::write(archive.path().join("unexpected.txt"), b"tamper").unwrap();

    let error = persistence
        .archive_revision(&metadata, &canonical)
        .unwrap_err();

    assert_eq!(error.code(), "console_corruption");
}

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
fn transaction_replace_rejects_changed_created_or_deleted_previous_yaml() {
    let new = b"providers:\n  - id: managed\n";
    for scenario in ["changed", "created", "deleted"] {
        let temp = TestDirectory::new(scenario);
        let routes = temp.path().join("routes.yaml");
        let previous_present = scenario != "created";
        if previous_present {
            fs::write(&routes, b"providers: []\n").unwrap();
        }
        let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
        let record = TransactionRecord::new_prepared(
            format!("tx-{scenario}"),
            Some("r0-000000000000"),
            "r1-111111111111",
            "revisions/r1-111111111111",
            previous_present,
            previous_present
                .then(|| sha256(b"providers: []\n"))
                .as_deref(),
            sha256(new),
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        )
        .unwrap();
        match scenario {
            "changed" => fs::write(&routes, b"operator changed\n").unwrap(),
            "created" => fs::write(&routes, b"operator created\n").unwrap(),
            "deleted" => fs::remove_file(&routes).unwrap(),
            _ => unreachable!(),
        }
        let before = fs::read(&routes).ok();

        let error = persistence
            .replace_routes_yaml_for_transaction(&record, new)
            .unwrap_err();

        assert_eq!(error.code(), "console_revision_conflict");
        assert_eq!(fs::read(&routes).ok(), before);
    }
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
fn journal_link_is_rejected_without_following_it() {
    let temp = TestDirectory::new("journal-link");
    let routes = temp.path().join("routes.yaml");
    let outside = temp.path().join("outside.ndjson");
    fs::write(&outside, b"outside-secret\n").unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    if let Err(error) = create_file_link(&outside, &persistence.journal_path()) {
        eprintln!("file links unavailable for journal contract: {error}");
        return;
    }
    let journal = TransactionJournal::new(persistence.clone());

    let error = journal.load_entries().unwrap_err();

    assert_eq!(error.code(), "console_journal_path_escape");
    assert_eq!(fs::read(&outside).unwrap(), b"outside-secret\n");
    assert!(persistence.is_read_only());
}

#[test]
fn backend_spy_receives_only_safe_existing_and_absent_replace_flags() {
    let calls = Arc::new(Mutex::new(Vec::<BackendCall>::new()));
    let backend = Arc::new(SpyBackend {
        calls: Arc::clone(&calls),
    });
    let temp = TestDirectory::new("backend-spy");
    let existing_routes = temp.path().join("existing.yaml");
    fs::write(&existing_routes, b"providers: []\n").unwrap();
    let existing = RouteConfigPersistence::with_backend(
        temp.path().join("existing-state").as_path(),
        &existing_routes,
        backend.clone(),
    )
    .unwrap();
    existing.replace_routes_yaml(b"aliases: {}\n").unwrap();

    let absent_routes = temp.path().join("absent.yaml");
    let absent = RouteConfigPersistence::with_backend(
        temp.path().join("absent-state").as_path(),
        &absent_routes,
        backend,
    )
    .unwrap();
    absent.replace_routes_yaml(b"providers: []\n").unwrap();

    let calls = calls.lock().unwrap().clone();
    let existing_call = calls
        .iter()
        .find(|call| matches!(call, BackendCall::Replace { .. }))
        .unwrap();
    match existing_call {
        BackendCall::Replace {
            destination,
            replacement,
            backup,
            flags,
            backup_existed,
        } => {
            assert_eq!(*flags, 0);
            assert!(!backup_existed);
            assert_eq!(destination.parent(), replacement.parent());
            assert_eq!(destination.parent(), backup.parent());
        }
        BackendCall::Move { .. } => unreachable!(),
    }
    let absent_call = calls
        .iter()
        .find(|call| matches!(call, BackendCall::Move { destination, .. } if destination == &absent_routes))
        .unwrap();
    match absent_call {
        BackendCall::Move {
            source,
            destination,
            flags,
        } => {
            assert_eq!(*flags, 0x8);
            assert_eq!(*flags & 0x1, 0);
            assert_eq!(source.parent(), destination.parent());
        }
        BackendCall::Replace { .. } => unreachable!(),
    }
}

#[test]
fn transaction_json_creation_uses_same_directory_atomic_move() {
    let calls = Arc::new(Mutex::new(Vec::<BackendCall>::new()));
    let backend = Arc::new(SpyBackend {
        calls: Arc::clone(&calls),
    });
    let temp = TestDirectory::new("transaction-json-atomic");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::with_backend(temp.path(), &routes, backend).unwrap();
    let journal = TransactionJournal::new(persistence);
    journal
        .persist(&transaction(
            "tx-json-atomic",
            "r1-111111111111",
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        ))
        .unwrap();
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| matches!(call, BackendCall::Move { destination, flags, .. } if destination.extension().and_then(|e| e.to_str()) == Some("json") && *flags == 0x8)));
}

#[test]
fn failed_existing_transaction_json_replace_enters_recovery_read_only() {
    let fail_replace = Arc::new(AtomicBool::new(false));
    let backend = Arc::new(SwitchableBackend {
        fail_replace: Arc::clone(&fail_replace),
    });
    let temp = TestDirectory::new("transaction-json-replace-failure");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::with_backend(temp.path(), &routes, backend).unwrap();
    let journal = TransactionJournal::new(persistence.clone());
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let mut record = transaction("tx-json-replace-failure", "r1-111111111111", now);
    journal.persist(&record).unwrap();
    record
        .transition(TransactionPhase::YamlReplaced, now, None)
        .unwrap();
    fail_replace.store(true, Ordering::Release);

    let error = journal.persist(&record).unwrap_err();

    assert_eq!(error.code(), "console_recovery_required");
    assert!(persistence.is_read_only());
    assert_eq!(
        journal.load_transaction(record.tx_id()).unwrap().phase(),
        TransactionPhase::Prepared
    );
}

#[cfg(windows)]
#[test]
fn windows_rejects_device_globalroot_and_ads_paths() {
    let temp = TestDirectory::new("windows-path-rejections");
    let state = temp.path().join("state");
    let device = RouteConfigPersistence::for_test(&state, Path::new(r"\\.\NUL"));
    assert_eq!(
        device.unwrap_err().code(),
        "console_unsupported_platform_operation"
    );
    let globalroot = RouteConfigPersistence::for_test(
        &state,
        Path::new(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1\routes.yaml"),
    );
    assert_eq!(
        globalroot.unwrap_err().code(),
        "console_unsupported_platform_operation"
    );
    let ads = RouteConfigPersistence::for_test(&state, &temp.path().join("routes.yaml:secret"));
    assert_eq!(
        ads.unwrap_err().code(),
        "console_unsupported_platform_operation"
    );
    for leaf in ["CON", "nul.txt", "COM1.yaml", "Lpt9", "routes. ", "routes."] {
        let rejected = RouteConfigPersistence::for_test(&state, &temp.path().join(leaf));
        assert_eq!(
            rejected.unwrap_err().code(),
            "console_unsupported_platform_operation",
            "Windows path leaf {leaf:?} must be rejected"
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_writer_lock_cannot_be_deleted_while_held() {
    let temp = TestDirectory::new("windows-lock-delete");
    let routes = temp.path().join("routes.yaml");
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let guard = persistence.try_writer_lock().unwrap();
    let error = fs::remove_file(persistence.writer_lock_path()).unwrap_err();
    assert!(
        matches!(
            error.kind(),
            io::ErrorKind::PermissionDenied | io::ErrorKind::Other
        ) || error.raw_os_error() == Some(32)
    );
    drop(guard);
}

#[cfg(windows)]
#[test]
fn windows_atomic_replace_supports_paths_longer_than_max_path() {
    let temp = TestDirectory::new("windows-long-path");
    let mut parent = temp.path().to_path_buf();
    for index in 0..12 {
        parent.push(format!("long-component-{index:02}-abcdef"));
    }
    fs::create_dir_all(&parent).unwrap();
    let routes = parent.join("routes.yaml");
    assert!(routes.as_os_str().len() > 260);
    let persistence =
        RouteConfigPersistence::for_test(&temp.path().join("state"), &routes).unwrap();

    persistence.replace_routes_yaml(b"providers: []\n").unwrap();
    persistence.replace_routes_yaml(b"aliases: {}\n").unwrap();

    assert_eq!(fs::read(routes).unwrap(), b"aliases: {}\n");
}

#[derive(Clone, Debug)]
enum BackendCall {
    Replace {
        destination: PathBuf,
        replacement: PathBuf,
        backup: PathBuf,
        flags: u32,
        backup_existed: bool,
    },
    Move {
        source: PathBuf,
        destination: PathBuf,
        flags: u32,
    },
}

#[derive(Debug)]
struct SpyBackend {
    calls: Arc<Mutex<Vec<BackendCall>>>,
}

#[derive(Debug)]
struct SwitchableBackend {
    fail_replace: Arc<AtomicBool>,
}

impl AtomicReplaceBackend for SwitchableBackend {
    fn replace_existing(
        &self,
        destination: &Path,
        replacement: &Path,
        backup: &Path,
        _flags: u32,
    ) -> io::Result<()> {
        if self.fail_replace.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected replace failure",
            ));
        }
        fs::copy(destination, backup)?;
        fs::remove_file(destination)?;
        fs::rename(replacement, destination)
    }

    fn move_new(&self, source: &Path, destination: &Path, _flags: u32) -> io::Result<()> {
        fs::rename(source, destination)
    }
}

impl AtomicReplaceBackend for SpyBackend {
    fn replace_existing(
        &self,
        destination: &Path,
        replacement: &Path,
        backup: &Path,
        flags: u32,
    ) -> io::Result<()> {
        let backup_existed = backup.exists();
        self.calls.lock().unwrap().push(BackendCall::Replace {
            destination: destination.to_path_buf(),
            replacement: replacement.to_path_buf(),
            backup: backup.to_path_buf(),
            flags,
            backup_existed,
        });
        fs::copy(destination, backup)?;
        fs::remove_file(destination)?;
        fs::rename(replacement, destination)
    }

    fn move_new(&self, source: &Path, destination: &Path, flags: u32) -> io::Result<()> {
        self.calls.lock().unwrap().push(BackendCall::Move {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            flags,
        });
        fs::rename(source, destination)
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

#[test]
fn writer_lock_is_an_os_lock_and_reports_a_stable_locked_error() {
    let temp = TestDirectory::new("writer-lock");
    let routes = temp.path().join("routes.yaml");
    let first = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let second = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    let guard = first.try_writer_lock().unwrap();

    let error = second.try_writer_lock().unwrap_err();

    assert_eq!(error.code(), "console_locked");
    drop(guard);
    second.try_writer_lock().unwrap();
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

fn transaction(tx_id: &str, revision: &str, now: OffsetDateTime) -> TransactionRecord {
    TransactionRecord::new_prepared(
        tx_id,
        Some("r0-000000000000"),
        revision,
        format!("revisions/{revision}"),
        true,
        Some("f03fc7b1589d54e15cf75e4d70821d7283996e6706596e4f5e9f7f1b5f74faef"),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        now,
    )
    .unwrap()
}

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

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
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

#[cfg(windows)]
fn create_file_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_file_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

use std::io::Write as _;
