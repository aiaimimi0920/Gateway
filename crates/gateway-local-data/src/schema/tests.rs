use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "gateway-schema-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn compatible_upgrade_and_repeat_keep_legacy_data_byte_identical() {
    let f = Fixture::new();
    let payload = b"legacy secret and unknown fields must survive";
    fs::write(f.0.join("routes.yaml"), payload).unwrap();
    upgrade(&f.0).unwrap();
    let modified = fs::metadata(f.0.join(VERSION_FILE))
        .unwrap()
        .modified()
        .unwrap();
    upgrade(&f.0).unwrap();
    assert_eq!(fs::read(f.0.join("routes.yaml")).unwrap(), payload);
    assert_eq!(version(&f.0).unwrap(), 1);
    assert_eq!(
        fs::metadata(f.0.join(VERSION_FILE))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
}

#[test]
fn failed_transform_restores_original_and_retry_completes() {
    let f = Fixture::new();
    upgrade(&f.0).unwrap();
    fs::write(f.0.join("credentials.json"), b"original").unwrap();
    let failed = Migration {
        from: 1,
        files: &["credentials.json", "new.json"],
        apply: |root| {
            fs::write(root.join("credentials.json"), b"partial")?;
            fs::write(root.join("new.json"), b"partial")?;
            Err(io::Error::other("injected migration failure"))
        },
    };
    assert!(run(&f.0, 2, &[failed]).is_err());
    assert_eq!(version(&f.0).unwrap(), 1);
    assert_eq!(fs::read(f.0.join("credentials.json")).unwrap(), b"original");
    assert!(!f.0.join("new.json").exists());
    let success = Migration {
        from: 1,
        files: &["credentials.json", "new.json"],
        apply: |root| fs::write(root.join("credentials.json"), b"upgraded"),
    };
    run(&f.0, 2, &[success]).unwrap();
    assert_eq!(version(&f.0).unwrap(), 2);
    assert_eq!(
        fs::read(f.0.join("migrations/v1-to-v2/0")).unwrap(),
        b"original"
    );
}

#[test]
fn interrupted_migration_is_restored_before_retry() {
    let f = Fixture::new();
    upgrade(&f.0).unwrap();
    fs::write(f.0.join("credentials.json"), b"original").unwrap();
    snapshot(
        &f.0,
        &f.0.join("migrations/v1-to-v2"),
        &["credentials.json"],
    )
    .unwrap();
    fs::write(f.0.join("credentials.json"), b"interrupted").unwrap();
    run(
        &f.0,
        2,
        &[Migration {
            from: 1,
            files: &["credentials.json"],
            apply: |root| {
                assert_eq!(fs::read(root.join("credentials.json"))?, b"original");
                Ok(())
            },
        }],
    )
    .unwrap();
}

#[test]
fn active_reader_prevents_incompatible_migration() {
    let f = Fixture::new();
    let _reader = upgrade(&f.0).unwrap();
    assert!(run(
        &f.0,
        2,
        &[Migration {
            from: 1,
            files: &[],
            apply: |_| Ok(())
        }]
    )
    .is_err());
    assert_eq!(version(&f.0).unwrap(), 1);
}

#[test]
fn newer_invalid_or_locked_storage_is_not_rewritten() {
    let f = Fixture::new();
    let oversized = format!("1{}invalid", " ".repeat(64));
    for text in ["99\n", "broken\n", oversized.as_str()] {
        fs::write(f.0.join(VERSION_FILE), text).unwrap();
        assert!(upgrade(&f.0).is_err());
        assert_eq!(fs::read_to_string(f.0.join(VERSION_FILE)).unwrap(), text);
    }
    fs::write(f.0.join(VERSION_FILE), "1\n").unwrap();
    let lock = OpenOptions::new()
        .write(true)
        .open(f.0.join("storage-migration.lock"))
        .unwrap();
    lock.lock().unwrap();
    assert!(upgrade(&f.0).is_err());
    assert!(owned_file(&f.0, "../outside").is_err());
}

#[test]
fn incomplete_snapshot_retry_does_not_keep_stale_absence_marker() {
    let f = Fixture::new();
    let backup = f.0.join("migrations/v1-to-v2");
    fs::create_dir_all(&backup).unwrap();
    fs::write(backup.join("0.absent"), b"").unwrap();
    fs::write(f.0.join("credentials.json"), b"original").unwrap();
    snapshot(&f.0, &backup, &["credentials.json"]).unwrap();
    fs::write(f.0.join("credentials.json"), b"partial").unwrap();
    restore(&f.0, &backup, &["credentials.json"]).unwrap();
    assert_eq!(fs::read(f.0.join("credentials.json")).unwrap(), b"original");
}
