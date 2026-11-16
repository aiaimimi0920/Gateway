use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "gateway-restore-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn files() -> BTreeMap<String, Vec<u8>> {
        [
            "local/routes.yaml",
            "local/state/runtime.sqlite3",
            "local/state/access-key-seal.json",
        ]
        .into_iter()
        .map(|name| (name.into(), b"new-data".to_vec()))
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn restore_is_staged_until_exclusive_startup_and_keeps_previous_data() {
    let root = Fixture::new();
    let lease = crate::prepare(&root.0).unwrap();
    fs::create_dir_all(root.0.join("local")).unwrap();
    fs::write(root.0.join("local/routes.yaml"), b"original").unwrap();
    stage(&root.0, &Fixture::files()).unwrap();
    assert_eq!(
        fs::read(root.0.join("local/routes.yaml")).unwrap(),
        b"original"
    );
    assert!(crate::prepare(&root.0).is_err());
    drop(lease);
    let lease = crate::prepare(&root.0).unwrap();
    assert_eq!(
        fs::read(root.0.join("local/routes.yaml")).unwrap(),
        b"new-data"
    );
    let archive = fs::read_dir(root.0.join("backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read(archive.join("previous/local/routes.yaml")).unwrap(),
        b"original"
    );
    drop(lease);
    assert!(crate::prepare(&root.0).is_ok());
}

#[test]
fn incomplete_preparation_is_resumed_without_overwriting_original_data() {
    let root = Fixture::new();
    drop(crate::prepare(&root.0).unwrap());
    stage(&root.0, &Fixture::files()).unwrap();
    fs::create_dir(root.0.join("restore-pending/previous")).unwrap();
    fs::write(root.0.join("restore-pending/previous/local.presence"), b"0").unwrap();
    let lease = crate::prepare(&root.0).unwrap();
    assert_eq!(
        fs::read(root.0.join("local/routes.yaml")).unwrap(),
        b"new-data"
    );
    drop(lease);
}

#[test]
fn interrupted_publication_rolls_back_before_allowing_readers() {
    let root = Fixture::new();
    drop(crate::prepare(&root.0).unwrap());
    fs::create_dir_all(root.0.join("local")).unwrap();
    fs::write(root.0.join("local/routes.yaml"), b"original").unwrap();
    stage(&root.0, &Fixture::files()).unwrap();
    let pending = root.0.join("restore-pending");
    let previous = pending.join("previous");
    fs::create_dir(&previous).unwrap();
    for unit in UNITS {
        fs::write(
            previous.join(format!("{unit}.presence")),
            if unit == "local" { b"1" } else { b"0" },
        )
        .unwrap();
    }
    fs::write(pending.join("started"), b"1").unwrap();
    fs::rename(root.0.join("local"), previous.join("local")).unwrap();
    fs::rename(pending.join("local"), root.0.join("local")).unwrap();
    assert!(crate::prepare(&root.0).is_err());
    assert_eq!(
        fs::read(root.0.join("local/routes.yaml")).unwrap(),
        b"original"
    );
    assert!(crate::prepare(&root.0).is_ok());
}

#[test]
fn cancelled_and_interrupted_staging_can_be_retried() {
    let root = Fixture::new();
    let lease = crate::prepare(&root.0).unwrap();
    fs::create_dir(root.0.join("restore-staging")).unwrap();
    fs::write(root.0.join("restore-staging/partial"), b"unfinished").unwrap();
    stage(&root.0, &Fixture::files()).unwrap();
    assert!(pending(&root.0).unwrap());
    cancel(&root.0).unwrap();
    assert!(!pending(&root.0).unwrap());
    assert!(!root.0.join("local/routes.yaml").exists());
    stage(&root.0, &Fixture::files()).unwrap();
    assert!(pending(&root.0).unwrap());
    drop(lease);
}

#[test]
fn traversal_case_collisions_and_file_directory_conflicts_are_rejected() {
    let root = Fixture::new();
    for path in [
        "local/../outside",
        "local/state:stream",
        "objects/CON",
        "objects/trailing.",
    ] {
        assert!(checked(&root.0, path).is_err());
    }
    let mut files = Fixture::files();
    files.insert("LOCAL/routes.yaml".into(), vec![]);
    assert!(validate_files(&files).is_err());
    let mut files = Fixture::files();
    files.insert("local/state".into(), vec![]);
    assert!(validate_files(&files).is_err());
    assert!(!root.0.join("restore-pending").exists());
}
