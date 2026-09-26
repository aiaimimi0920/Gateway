//! Managed YAML snapshots preserve backups, immutable revisions and compare-before-replace.

use super::{sha256, TestDirectory};
use neuro_gateway::console::document::canonicalize_route_document;
use neuro_gateway::console::persistence::{
    FirstSavePresence, RouteConfigPersistence, TransactionRecord,
};
use neuro_gateway::console::revision::{RevisionActor, RevisionMetadata};
use neuro_gateway::routing::config::RouteConfigYaml;
use std::fs;
use time::OffsetDateTime;

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
