//! Cancellation must not admit another writer until actual blocking I/O finishes.
use super::*;

#[tokio::test]
async fn maintenance_cancelled_blocking_write_keeps_process_and_file_admission() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-maintenance-blocking-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("lock");
    let file = std::fs::File::create(&path).unwrap();
    file.try_lock().unwrap();
    let process = Arc::new(tokio::sync::Mutex::new(()));
    let lease = Arc::new(MaintenanceAdmission {
        _process: process.clone().try_lock_owned().unwrap(),
        _shared: admission::ProviderAdmissionGuard::Local { _file: file },
    });
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let task = tokio::spawn(blocking_preservation(lease, move || {
        let _ = entered.send(());
        released.recv_timeout(Duration::from_secs(5))?;
        Ok(())
    }));
    started.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(process.try_lock().is_err());
    let contender = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    assert!(contender.try_lock().is_err());
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        // A zero Arc strong count is observable before its fields finish dropping.
        // Wait for the actual file admission, not Weak::upgrade as a destructor fence.
        loop {
            match contender.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => tokio::task::yield_now().await,
                Err(error) => panic!("maintenance admission release failed: {error:?}"),
            }
        }
    })
    .await
    .unwrap();
    assert!(process.try_lock().is_ok());
    drop(contender);
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
