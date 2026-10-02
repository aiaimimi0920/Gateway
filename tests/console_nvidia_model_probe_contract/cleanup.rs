//! Only isolated probe fixtures may wait for final Windows SQLite handles to release.
use std::{io, path::Path, time::Duration};

pub(super) async fn remove_probe_directory(directory: &Path) -> io::Result<()> {
    assert!(directory.starts_with(std::env::temp_dir()));
    assert!(directory
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("nvidia-model-probe-")));
    remove_with_retry(|| std::fs::remove_dir_all(directory), cfg!(windows)).await
}

async fn remove_with_retry(
    mut remove: impl FnMut() -> io::Result<()>,
    windows: bool,
) -> io::Result<()> {
    for attempt in 0..40 {
        match remove() {
            Err(error)
                if windows && matches!(error.raw_os_error(), Some(32 | 33)) && attempt < 39 =>
            {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            result => return result,
        }
    }
    unreachable!("the final attempt always returns its result")
}

#[tokio::test]
async fn sharing_and_lock_violations_retry_until_removed() {
    let mut attempts = 0;
    remove_with_retry(
        || {
            attempts += 1;
            match attempts {
                1 => Err(io::Error::from_raw_os_error(32)),
                2 => Err(io::Error::from_raw_os_error(33)),
                _ => Ok(()),
            }
        },
        true,
    )
    .await
    .unwrap();
    assert_eq!(attempts, 3);
}

#[tokio::test]
async fn persistent_lock_error_is_returned_after_bounded_attempts() {
    let mut attempts = 0;
    let error = remove_with_retry(
        || {
            attempts += 1;
            Err(io::Error::from_raw_os_error(32))
        },
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(attempts, 40);
    assert_eq!(error.raw_os_error(), Some(32));
}

#[tokio::test]
async fn unrelated_errors_and_non_windows_errors_are_not_retried() {
    for (windows, code) in [(true, 5), (true, 2), (false, 32), (false, 33)] {
        let mut attempts = 0;
        let error = remove_with_retry(
            || {
                attempts += 1;
                Err(io::Error::from_raw_os_error(code))
            },
            windows,
        )
        .await
        .unwrap_err();
        assert_eq!(attempts, 1);
        assert_eq!(error.raw_os_error(), Some(code));
    }
}

#[cfg(windows)]
#[tokio::test]
async fn real_windows_open_handle_prevents_cleanup_until_released() {
    use std::os::windows::fs::OpenOptionsExt;
    let directory =
        std::env::temp_dir().join(format!("nvidia-model-probe-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("fixture.sqlite3");
    std::fs::write(&file, b"synthetic fixture").unwrap();
    let mut held = Some(
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(file)
            .unwrap(),
    );
    let mut attempts = 0;
    remove_with_retry(
        || {
            attempts += 1;
            if attempts == 2 {
                drop(held.take());
            }
            std::fs::remove_dir_all(&directory)
        },
        true,
    )
    .await
    .unwrap();
    assert_eq!(attempts, 2);
    assert!(!directory.exists());
}
