use super::*;
use tokio::sync::Semaphore;

fn lease() -> Arc<OwnedSemaphorePermit> {
    Arc::new(Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap())
}

#[test]
fn gemini_canvas_encoder_workspace_is_unique_and_cleans_only_owned_files() {
    let first = EncoderWorkspace::create(b"same source", "png", lease()).unwrap();
    let second = EncoderWorkspace::create(b"same source", "png", lease()).unwrap();
    let first_directory = first.directory.clone();
    let second_directory = second.directory.clone();
    assert_ne!(first_directory, second_directory);
    assert_eq!(fs::read(&first.source).unwrap(), b"same source");
    assert!(!first.output.exists());
    assert!(first.read_output().is_err());
    fs::write(&first.output, b"encoded").unwrap();
    assert_eq!(first.read_output().unwrap(), b"encoded");
    drop(first);
    assert!(!first_directory.exists());
    assert_eq!(fs::read(&second.source).unwrap(), b"same source");
    drop(second);
    assert!(!second_directory.exists());
}

#[test]
fn gemini_canvas_encoder_workspace_rejects_unsafe_extensions_and_large_source() {
    for extension in ["../png", "C:\\image", "png:stream", "", "/png", "PNG"] {
        assert!(EncoderWorkspace::create(b"source", extension, lease()).is_err());
    }
    assert!(EncoderWorkspace::create(&vec![0; MAX_SOURCE_BYTES + 1], "png", lease()).is_err());
}

#[test]
fn gemini_canvas_encoder_workspace_rejects_empty_large_and_nonregular_output() {
    let workspace = EncoderWorkspace::create(b"source", "png", lease()).unwrap();
    fs::write(&workspace.output, b"").unwrap();
    assert!(workspace.read_output().is_err());
    File::create(&workspace.output)
        .unwrap()
        .set_len(MAX_ENCODED_BYTES as u64 + 1)
        .unwrap();
    assert!(workspace.read_output().is_err());
    fs::remove_file(&workspace.output).unwrap();
    fs::create_dir(&workspace.output).unwrap();
    assert!(workspace.read_output().is_err());
    fs::remove_dir(&workspace.output).unwrap();
}

#[test]
fn gemini_canvas_encoder_workspace_lease_survives_until_last_owner() {
    let slots = Arc::new(Semaphore::new(1));
    let permit = Arc::new(slots.clone().try_acquire_owned().unwrap());
    let workspace = Arc::new(EncoderWorkspace::create(b"source", "png", permit).unwrap());
    let directory = workspace.directory.clone();
    let reader = workspace.clone();
    drop(workspace);
    assert!(directory.exists());
    assert_eq!(slots.available_permits(), 0);
    drop(reader);
    assert!(!directory.exists());
    assert_eq!(slots.available_permits(), 1);
}

#[test]
fn gemini_canvas_encoder_workspace_creation_failure_releases_lease() {
    let parent = EncoderWorkspace::create(b"source", "png", lease()).unwrap();
    let slots = Arc::new(Semaphore::new(1));
    let permit = Arc::new(slots.clone().try_acquire_owned().unwrap());
    assert!(EncoderWorkspace::create_in(&parent.source, b"source", "png", permit).is_err());
    assert_eq!(slots.available_permits(), 1);
    assert_eq!(fs::read(&parent.source).unwrap(), b"source");
}

#[tokio::test]
async fn gemini_canvas_encoder_workspace_cancelled_blocking_owner_cleans_after_io() {
    let slots = Arc::new(Semaphore::new(1));
    let permit = Arc::new(slots.clone().try_acquire_owned().unwrap());
    let workspace = Arc::new(EncoderWorkspace::create(b"source", "png", permit).unwrap());
    let directory = workspace.directory.clone();
    fs::write(&workspace.output, b"encoded").unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let owner = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            released
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            workspace.read_output().unwrap()
        })
        .await
        .unwrap()
    });
    ready.await.unwrap();
    owner.abort();
    assert!(owner.await.unwrap_err().is_cancelled());
    assert!(directory.exists());
    assert_eq!(slots.available_permits(), 0);
    release.send(()).unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while slots.available_permits() == 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "blocking owner did not release lease"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!directory.exists());
}

#[test]
fn gemini_canvas_encoder_workspace_preserves_unrecognized_entries() {
    let workspace = EncoderWorkspace::create(b"source", "png", lease()).unwrap();
    let directory = workspace.directory.clone();
    let unknown = directory.join("unrecognized");
    fs::write(&unknown, b"preserve").unwrap();
    drop(workspace);
    assert_eq!(fs::read(&unknown).unwrap(), b"preserve");
    fs::remove_file(unknown).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn gemini_canvas_encoder_workspace_cleanup_does_not_block_runtime_or_release_early() {
    let slots = Arc::new(Semaphore::new(1));
    let permit = slots.clone().try_acquire_owned().unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let (finished, completion) = tokio::sync::oneshot::channel();
    dispatch_workspace_cleanup(move || {
        started.send(()).unwrap();
        released
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        drop(permit);
        finished.send(()).unwrap();
    });
    ready.await.unwrap();
    // This test uses a single-thread runtime; reaching here requires off-thread dispatch.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert_eq!(slots.available_permits(), 0);
    release.send(()).unwrap();
    completion.await.unwrap();
    assert_eq!(slots.available_permits(), 1);
}

#[test]
fn gemini_canvas_encoder_workspace_cleanup_without_runtime_is_inline() {
    let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let marker = completed.clone();
    dispatch_workspace_cleanup(move || marker.store(true, std::sync::atomic::Ordering::SeqCst));
    assert!(completed.load(std::sync::atomic::Ordering::SeqCst));
}

#[cfg(unix)]
#[test]
fn gemini_canvas_encoder_workspace_rejects_symlink_output_and_is_private() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let workspace = EncoderWorkspace::create(b"source", "png", lease()).unwrap();
    assert_eq!(
        fs::metadata(&workspace.directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&workspace.source)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    symlink(&workspace.source, &workspace.output).unwrap();
    assert!(workspace.read_output().is_err());
}
