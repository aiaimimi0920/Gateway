use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::ThreadId;
use std::time::Duration;

use tokio::sync::oneshot;

use super::NativeWatchOwner;

mod lifetime;

struct Resource {
    release: Option<mpsc::Receiver<()>>,
    observed: mpsc::Sender<(ThreadId, bool)>,
}

impl Drop for Resource {
    fn drop(&mut self) {
        let progressed = self.release.as_ref().map_or(true, |wait| {
            wait.recv_timeout(Duration::from_secs(2)).is_ok()
        });
        let _ = self
            .observed
            .send((std::thread::current().id(), progressed));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn native_setup_does_not_stall_executor() {
    let (entered_tx, entered) = oneshot::channel();
    let (release, wait) = mpsc::channel();
    let progressed = Arc::new(AtomicBool::new(false));
    let worker_progressed = progressed.clone();
    let mut owner = NativeWatchOwner::spawn(move || {
        entered_tx.send(()).unwrap();
        worker_progressed.store(
            wait.recv_timeout(Duration::from_secs(2)).is_ok(),
            Ordering::SeqCst,
        );
        Some(((), true))
    })
    .unwrap();
    entered.await.unwrap();
    let probe = tokio::spawn(async move {
        let _ = release.send(());
    });
    assert_eq!(owner.ready().await.unwrap(), Some(true));
    owner.shutdown().await.unwrap();
    probe.await.unwrap();
    assert!(
        progressed.load(Ordering::SeqCst),
        "native setup must allow async progress"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn native_destruction_does_not_stall_executor() {
    let (release, wait) = mpsc::channel();
    let (observed, result) = mpsc::channel();
    let mut owner = NativeWatchOwner::spawn(move || {
        Some((
            Resource {
                release: Some(wait),
                observed,
            },
            true,
        ))
    })
    .unwrap();
    owner.ready().await.unwrap();
    let cleanup = tokio::spawn(owner.shutdown());
    let probe = tokio::spawn(async move {
        let _ = release.send(());
    });
    cleanup.await.unwrap().unwrap();
    probe.await.unwrap();
    assert!(
        result.recv_timeout(Duration::from_secs(1)).unwrap().1,
        "native destruction must allow async progress"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn native_shutdown_deadline_returns_while_worker_finishes_cleanup() {
    let (release, wait) = mpsc::channel();
    let (observed, destroyed) = mpsc::channel();
    let mut owner = NativeWatchOwner::spawn(move || {
        Some((
            Resource {
                release: Some(wait),
                observed,
            },
            true,
        ))
    })
    .unwrap();
    owner.ready().await.unwrap();
    let error = owner
        .shutdown_with_deadline(Duration::from_millis(10))
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_watch_owner_deadline_exceeded")
    );
    assert!(destroyed.recv_timeout(Duration::from_millis(50)).is_err());
    release.send(()).unwrap();
    assert!(destroyed.recv_timeout(Duration::from_secs(3)).is_ok());
}

#[tokio::test]
async fn native_readiness_deadline_preserves_late_setup_cleanup() {
    let (entered_tx, entered) = oneshot::channel();
    let (release, wait) = mpsc::channel();
    let (destroyed, observed) = mpsc::channel();
    let mut owner = NativeWatchOwner::spawn(move || {
        entered_tx.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(3)).unwrap();
        Some((
            Resource {
                release: None,
                observed: destroyed,
            },
            true,
        ))
    })
    .unwrap();
    entered.await.unwrap();

    let error = owner
        .ready_with_deadline(Duration::from_millis(10))
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_watch_owner_deadline_exceeded")
    );

    let error = owner
        .shutdown_with_deadline(Duration::from_millis(10))
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_watch_owner_deadline_exceeded")
    );
    assert!(observed.recv_timeout(Duration::from_millis(50)).is_err());

    release.send(()).unwrap();
    assert!(observed.recv_timeout(Duration::from_secs(3)).is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn native_setup_and_destruction_share_non_executor_thread() {
    let executor = std::thread::current().id();
    let (created_tx, created) = oneshot::channel();
    let (observed, destroyed) = mpsc::channel();
    let mut owner = NativeWatchOwner::spawn(move || {
        created_tx.send(std::thread::current().id()).unwrap();
        Some((
            Resource {
                release: None,
                observed,
            },
            true,
        ))
    })
    .unwrap();
    assert_eq!(owner.ready().await.unwrap(), Some(true));
    assert!(owner.is_running());
    let created = created.await.unwrap();
    owner.shutdown().await.unwrap();
    let destroyed = destroyed.recv_timeout(Duration::from_secs(1)).unwrap().0;
    assert_ne!(created, executor);
    assert_eq!(created, destroyed);
}

#[test]
fn native_owner_does_not_require_a_blocking_pool_slot() {
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    executor.block_on(async {
        let (occupied_tx, occupied) = oneshot::channel();
        let (release, wait) = mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            occupied_tx.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(3)).unwrap();
        });
        occupied.await.unwrap();
        let mut owner = NativeWatchOwner::spawn(|| Some(((), true))).unwrap();
        let ready = tokio::time::timeout(Duration::from_millis(250), owner.ready()).await;
        release.send(()).unwrap();
        blocker.await.unwrap();
        owner.shutdown().await.unwrap();
        assert_eq!(ready.unwrap().unwrap(), Some(true));
    });
}

#[tokio::test]
async fn failed_initialization_reports_no_resources() {
    let mut owner = NativeWatchOwner::spawn(|| None::<((), bool)>).unwrap();
    assert_eq!(owner.ready().await.unwrap(), None);
    assert!(!owner.is_running());
    owner.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_panic_reports_fixed_error_without_hanging_readiness_or_cleanup() {
    let mut owner = NativeWatchOwner::spawn(|| -> Option<((), bool)> {
        panic!("fixture native setup panic");
    })
    .unwrap();
    let error = tokio::time::timeout(Duration::from_secs(3), owner.ready())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_watch_owner_failed")
    );
    let error = tokio::time::timeout(Duration::from_secs(3), owner.shutdown())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_watch_owner_failed")
    );
}
