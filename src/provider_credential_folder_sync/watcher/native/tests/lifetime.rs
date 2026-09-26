use super::*;
use crate::provider_credential_folder_sync::watcher::{
    mailbox::channel, start_folder_sync_watchers,
};
use crate::state::ProviderCredentialFolderSyncRuntime;

#[tokio::test(flavor = "current_thread")]
async fn cancelled_setup_drops_late_resources_on_native_owner() {
    let (entered_tx, entered) = oneshot::channel();
    let (release, wait) = mpsc::channel();
    let (observed, destroyed) = mpsc::channel();
    let completed = Arc::new(AtomicBool::new(false));
    let worker_completed = completed.clone();
    let owner = NativeWatchOwner::spawn(move || {
        entered_tx.send(()).unwrap();
        worker_completed.store(
            wait.recv_timeout(Duration::from_secs(2)).is_ok(),
            Ordering::SeqCst,
        );
        Some((
            Resource {
                release: None,
                observed,
            },
            true,
        ))
    })
    .unwrap();
    entered.await.unwrap();
    drop(owner);
    release.send(()).ok();
    let destroyed = destroyed.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(
        completed.load(Ordering::SeqCst),
        "caller can cancel while setup is pending"
    );
    assert_ne!(destroyed.0, std::thread::current().id());
    assert_eq!(Arc::strong_count(&completed), 1);
}

#[test]
fn tokio_runtime_shutdown_releases_native_owner() {
    let (observed, destroyed) = mpsc::channel();
    let helper = std::thread::spawn(move || {
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        executor.block_on(async {
            let mut owner = NativeWatchOwner::spawn(move || {
                Some((
                    Resource {
                        release: None,
                        observed,
                    },
                    true,
                ))
            })
            .unwrap();
            owner.ready().await.unwrap();
            tokio::spawn(async move {
                let _owner = owner;
                std::future::pending::<()>().await;
            });
        });
        drop(executor);
    });
    let helper_id = helper.thread().id();
    helper.join().unwrap();
    let destroyed = destroyed.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_ne!(destroyed.0, helper_id);
}

#[tokio::test(flavor = "current_thread")]
async fn real_native_and_poll_callbacks_are_released_after_shutdown() {
    let root = std::env::temp_dir().join(format!(
        "gateway-folder-native-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir(&root).unwrap();
    let directory = Directory(root.clone());
    let (signal, mut receiver) = channel(ProviderCredentialFolderSyncRuntime::new(true));
    let mut owner = NativeWatchOwner::spawn(move || {
        let handles = start_folder_sync_watchers(&root, signal, 250);
        let running = handles.is_running();
        Some((handles, running))
    })
    .unwrap();
    let ready = owner.ready().await.unwrap();
    owner.shutdown().await.unwrap();
    // notify 6.1.1 detaches several backend threads; drain their callback senders too.
    tokio::time::timeout(Duration::from_secs(3), async {
        while receiver.recv().await.is_some() {}
    })
    .await
    .expect("all native and poll callback senders released");
    assert_eq!(ready, Some(true));
    drop(directory);
}

struct Directory(std::path::PathBuf);

impl Drop for Directory {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(self
            .0
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gateway-folder-native-"));
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
