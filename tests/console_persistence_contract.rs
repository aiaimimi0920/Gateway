//! Persistence contracts share isolated directories and stable transaction fixtures.

#[path = "console_persistence_contract/yaml_snapshots.rs"]
mod yaml_snapshots;

#[path = "console_persistence_contract/journal.rs"]
mod journal;

#[path = "console_persistence_contract/recovery.rs"]
mod recovery;

#[path = "console_persistence_contract/atomic_backend.rs"]
mod atomic_backend;

#[path = "console_persistence_contract/platform_paths.rs"]
mod platform_paths;

use std::fs;
use std::path::{Path, PathBuf};

use neuro_gateway::console::persistence::TransactionRecord;
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

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
