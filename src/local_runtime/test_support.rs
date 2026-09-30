//! Windows SQLite workers may release their final file handle after pool shutdown.
pub(crate) async fn remove_test_root(root: &std::path::Path) {
    assert!(root.starts_with(std::env::temp_dir()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("gateway-local-"));
    for attempt in 0..40 {
        match std::fs::remove_dir_all(root) {
            Ok(()) => return,
            Err(error)
                if attempt < 39
                    && (error.raw_os_error() == Some(32)
                        || error.kind() == std::io::ErrorKind::PermissionDenied) =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            Err(error) => panic!("Cannot remove isolated local runtime test root: {error}"),
        }
    }
}
