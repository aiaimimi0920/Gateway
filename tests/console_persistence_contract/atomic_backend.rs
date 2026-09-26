//! Replacement spies and injected failures verify native flags and transaction publication.

use super::{transaction, TestDirectory};
use neuro_gateway::console::journal::TransactionJournal;
use neuro_gateway::console::persistence::{
    AtomicReplaceBackend, RouteConfigPersistence, TransactionPhase,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

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
            assert_eq!(
                *backup_existed,
                cfg!(not(windows)),
                "Windows ReplaceFileW requires an absent backup path, while non-Windows stages the previous contents before invoking the backend"
            );
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
