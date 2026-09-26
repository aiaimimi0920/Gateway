//! Native path and writer-lock contracts preserve platform-specific safety boundaries.

use super::TestDirectory;
use neuro_gateway::console::journal::TransactionJournal;
use neuro_gateway::console::persistence::RouteConfigPersistence;
use std::fs;
use std::io;
use std::path::Path;

#[cfg(windows)]
fn create_file_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_file_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
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
