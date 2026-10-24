use super::*;
use std::io::Write;

struct TempDirectory(std::path::PathBuf);
impl TempDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("gateway-diagnostics-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config(root: &std::path::Path) -> ConsoleConfig {
    ConsoleConfig {
        state_dir: root.join("state"),
        log_source: Some(root.join("logs/gateway-runtime-current.log")),
        ..Default::default()
    }
}

#[test]
fn settings_roundtrip_and_corrupt_file_are_not_overwritten_on_read() {
    let root = TempDirectory::new();
    let config = config(root.path());
    assert_eq!(settings(&config).unwrap().policy, LogPolicy::default());
    save_policy(
        &config,
        LogPolicy {
            retention_days: 7,
            log_limit_mb: 100,
        },
    )
    .unwrap();
    assert_eq!(load_policy(&config).unwrap().retention_days, 7);
    let path = policy_path(&config);
    fs::write(&path, "corrupt").unwrap();
    assert!(load_policy(&config).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "corrupt");
}

#[test]
fn exports_only_logs_and_drops_partial_secret_records() {
    let root = TempDirectory::new();
    let config = config(root.path());
    let directory = directory(&config).unwrap();
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("credentials.json"), "do-not-export").unwrap();
    fs::create_dir(directory.join("gateway-runtime-directory.log")).unwrap();
    let mut file = fs::File::create(config.log_source.as_ref().unwrap()).unwrap();
    file.write_all(b"token=").unwrap();
    file.write_all(&vec![b'x'; files::MAX_FILE_BYTES as usize + 100])
        .unwrap();
    file.write_all(b"\nsafe final line\n").unwrap();
    drop(file);
    let export = export_logs(&config).unwrap();
    assert!(export.truncated);
    assert!(export.content.contains("safe final line"));
    assert!(!export.content.contains("xxxx"));
    assert!(!export.content.contains("do-not-export"));
    assert!(export.content.len() < 1024);
}

#[test]
fn cleanup_preserves_current_and_unowned_files() {
    let root = TempDirectory::new();
    let config = config(root.path());
    let directory = directory(&config).unwrap();
    fs::create_dir_all(&directory).unwrap();
    let history = directory.join("gateway-runtime-old.log");
    fs::File::create(&history)
        .unwrap()
        .set_len(101 * 1024 * 1024)
        .unwrap();
    let current = config.log_source.as_ref().unwrap();
    fs::write(current, "current").unwrap();
    fs::write(directory.join("other.log"), "unowned").unwrap();
    retention::clean(&directory, current, &LogPolicy::default()).unwrap();
    assert!(history.exists());
    retention::clean(
        &directory,
        current,
        &LogPolicy {
            retention_days: 0,
            log_limit_mb: 100,
        },
    )
    .unwrap();
    assert!(!history.exists());
    assert!(current.exists());
    assert!(directory.join("other.log").exists());
}

#[test]
fn active_writer_is_not_deleted() {
    let root = TempDirectory::new();
    let config = config(root.path());
    let directory = directory(&config).unwrap();
    fs::create_dir_all(&directory).unwrap();
    let history = directory.join("gateway-runtime-active.log");
    fs::File::create(&history)
        .unwrap()
        .set_len(101 * 1024 * 1024)
        .unwrap();
    let mut options = fs::OpenOptions::new();
    options.append(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(3);
    }
    let file = options.open(&history).unwrap();
    #[cfg(unix)]
    file.try_lock().unwrap();
    retention::clean(
        &directory,
        config.log_source.as_ref().unwrap(),
        &LogPolicy {
            retention_days: 0,
            log_limit_mb: 100,
        },
    )
    .unwrap();
    assert!(history.exists());
}

#[test]
fn rejects_hard_links_instead_of_exporting_an_unrelated_file() {
    let root = TempDirectory::new();
    let target = root.path().join("secret");
    fs::write(&target, "never-export").unwrap();
    let link = root.path().join("gateway-runtime-linked.log");
    fs::hard_link(&target, &link).unwrap();
    assert!(files::tail(&link).is_err());
}

#[cfg(unix)]
#[test]
fn refuses_links_and_special_files() {
    let root = TempDirectory::new();
    let target = root.path().join("secret");
    fs::write(&target, "never-export").unwrap();
    let link = root.path().join("gateway-runtime-link.log");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(files::tail(&link).is_err());
    let linked_dir = root.path().join("linked");
    std::os::unix::fs::symlink(root.path(), &linked_dir).unwrap();
    assert!(files::tail(&linked_dir.join("secret")).is_err());
}
