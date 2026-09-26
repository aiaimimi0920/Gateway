#[cfg(test)]
mod persistence_lock_contract {
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::sync::Arc;

    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::console::document::canonicalize_route_document;
    use crate::console::journal::{JournalAppendBackend, TransactionJournal};
    use crate::console::persistence::{RouteConfigPersistence, TransactionRecord};
    use crate::console::revision::{RevisionActor, RevisionMetadata};
    use crate::routing::config::RouteConfigYaml;

    #[test]
    fn one_writer_guard_spans_prepared_replace_and_phase_persistence() {
        let root =
            std::env::temp_dir().join(format!("gateway-console-locked-api-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        fs::write(&routes, b"providers: []\n").unwrap();
        let persistence = RouteConfigPersistence::for_test(&root, &routes).unwrap();
        let journal = TransactionJournal::new(persistence.clone());
        let document: RouteConfigYaml = serde_yaml::from_str(
            "providers: []\nmodel_routes: []\naliases:\n  managed: managed-model\n",
        )
        .unwrap();
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
        let old_digest = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(b"providers: []\n"))
        };
        let record = TransactionRecord::new_prepared(
            "tx-one-guard",
            Some("r0-000000000000"),
            metadata.id(),
            format!("revisions/{}", metadata.id()),
            true,
            Some(&old_digest),
            canonical.yaml_digest(),
            now,
        )
        .unwrap();

        let guard = persistence.try_writer_lock().unwrap();
        persistence
            .archive_revision_locked(&guard, &metadata, &canonical)
            .unwrap();
        journal.persist_locked(&guard, &record).unwrap();
        let receipt = persistence
            .replace_routes_yaml_for_transaction_locked(&guard, &record, canonical.canonical_yaml())
            .unwrap();
        journal
            .record_yaml_replaced_locked(&guard, record.tx_id(), &receipt, now)
            .unwrap();

        assert_eq!(fs::read(&routes).unwrap(), canonical.canonical_yaml());
        assert_eq!(
            persistence.try_writer_lock().unwrap_err().code(),
            "console_locked"
        );
        drop(guard);
        let _ = fs::remove_dir_all(root);
    }

    #[derive(Debug)]
    struct FailingJournalAppend;

    impl JournalAppendBackend for FailingJournalAppend {
        fn append_and_sync(&self, _path: &Path, _bytes: &[u8]) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected journal failure",
            ))
        }
    }

    #[test]
    fn journal_append_failure_after_transaction_json_forces_read_only_recovery() {
        let root = std::env::temp_dir().join(format!(
            "gateway-console-journal-failure-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        let persistence = RouteConfigPersistence::for_test(&root, &routes).unwrap();
        let journal = TransactionJournal::with_append_backend_for_test(
            persistence.clone(),
            Arc::new(FailingJournalAppend),
        );
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let record = TransactionRecord::new_prepared(
            "tx-journal-failure",
            Some("r0-000000000000"),
            "r1-111111111111",
            "revisions/r1-111111111111",
            false,
            None,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            now,
        )
        .unwrap();

        let error = journal.persist(&record).unwrap_err();

        assert_eq!(error.code(), "console_recovery_required");
        assert!(persistence.is_read_only());
        assert!(persistence
            .transaction_record_path(record.tx_id())
            .unwrap()
            .is_file());
        assert!(!persistence.journal_path().exists());
        let _ = fs::remove_dir_all(root);
    }
}
