//! Scheduling contracts use bounded native waits so the inline baseline cannot hang.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::task::Poll;
use std::time::Duration;

use tokio::sync::oneshot;

use super::{run_owned, wait_for_run_release, ProviderCredentialFolderSyncRuntime};

#[tokio::test(flavor = "current_thread")]
async fn blocking_operation_does_not_stall_async_executor() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let (started_tx, started) = oneshot::channel();
    let (progress_tx, progress_rx) = mpsc::channel();
    let mut request = Box::pin(run_owned(&runtime, || async move {
        started_tx.send(()).unwrap();
        // Models a blocked native call. The OS deadline also releases the old inline task.
        Ok(progress_rx.recv_timeout(Duration::from_secs(2)).is_ok())
    }));
    assert!(matches!(futures::poll!(request.as_mut()), Poll::Pending));
    started.await.unwrap();
    let probe = tokio::spawn(async move {
        let _ = progress_tx.send(());
    });
    let progressed = request.await.unwrap();
    probe.await.unwrap();
    assert!(
        progressed,
        "async work must run while native work is blocked"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn worker_drives_async_timers_away_from_executor_thread() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let executor_thread = std::thread::current().id();
    let worker_threads = run_owned(&runtime, || async {
        let before = std::thread::current().id();
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok((before, std::thread::current().id()))
    })
    .await
    .unwrap();
    assert_ne!(worker_threads.0, executor_thread);
    assert_eq!(worker_threads.0, worker_threads.1);
}

#[test]
fn dedicated_worker_keeps_admission_without_shared_pool_queueing() {
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    executor.block_on(async {
        let runtime = ProviderCredentialFolderSyncRuntime::new(true);
        let (occupied_tx, occupied) = oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            occupied_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        occupied.await.unwrap();
        let effects = Arc::new(AtomicUsize::new(0));
        let owned_effects = effects.clone();
        let (started_tx, started) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let mut request = Box::pin(run_owned(&runtime, || async move {
            owned_effects.fetch_add(1, Ordering::SeqCst);
            started_tx.send(()).unwrap();
            finish_rx.await.unwrap();
            Ok(())
        }));
        assert!(matches!(futures::poll!(request.as_mut()), Poll::Pending));
        tokio::time::timeout(Duration::from_millis(250), started)
            .await
            .expect("dedicated worker must start while shared pool is occupied")
            .unwrap();
        runtime.set_enabled(false);
        assert!(runtime.try_begin_sync_run().is_none());
        drop(request);
        finish_tx.send(()).unwrap();
        wait_for_run_release(&runtime).await;
        assert_eq!(effects.load(Ordering::SeqCst), 1);
        assert_eq!(Arc::strong_count(&effects), 1);
        // Always release/join the pool blocker after proving the run did not queue there.
        release_tx.send(()).unwrap();
        blocker.await.unwrap();
    });
}
