//! Runtime shutdown must wake async waits, but cannot preempt a native call.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::task::Poll;
use std::time::Duration;

use tokio::sync::oneshot;

use super::run_owned;
use crate::state::ProviderCredentialFolderSyncRuntime;

#[test]
fn runtime_shutdown_releases_worker_waiting_on_in_memory_future() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let owned_runtime = runtime.clone();
    let effects = Arc::new(AtomicUsize::new(0));
    let owned_effects = effects.clone();
    let (release, wait) = oneshot::channel();
    let (ready_tx, ready) = mpsc::channel();
    let (exited_tx, exited) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        executor.block_on(async {
            let (entered_tx, entered) = mpsc::channel();
            let mut request = Box::pin(run_owned(&owned_runtime, || async move {
                entered_tx.send(()).unwrap();
                let _ = wait.await;
                owned_effects.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }));
            assert!(matches!(futures::poll!(request.as_mut()), Poll::Pending));
            // Do not yield this current-thread executor: shutdown must also own a
            // monitor that was spawned but has never been polled.
            entered.recv_timeout(Duration::from_secs(3)).unwrap();
            drop(request);
        });
        ready_tx.send(()).unwrap();
        drop(executor);
        exited_tx.send(()).unwrap();
    });
    let admitted = ready.recv_timeout(Duration::from_secs(3));
    let stopped = exited.recv_timeout(Duration::from_secs(1));
    // Rescue a faulty worker before joining, so the regression cannot hang the suite.
    let _ = release.send(());
    thread.join().unwrap();
    admitted.unwrap();
    stopped.expect("runtime shutdown must cancel an async wait in the blocking worker");
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert_eq!(Arc::strong_count(&effects), 1);
    assert!(runtime.try_begin_sync_run().is_some());
}

#[test]
fn runtime_shutdown_retains_permit_until_native_call_finishes() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let owned_runtime = runtime.clone();
    let effects = Arc::new(AtomicUsize::new(0));
    let owned_effects = effects.clone();
    let (release, wait) = mpsc::channel();
    let (ready_tx, ready) = mpsc::channel();
    let (exited_tx, exited) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let executor = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        executor.block_on(async {
            let (entered_tx, entered) = mpsc::channel();
            let mut request = Box::pin(run_owned(&owned_runtime, || async move {
                entered_tx.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                owned_effects.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }));
            assert!(matches!(futures::poll!(request.as_mut()), Poll::Pending));
            // Keep the monitor unpolled while the independent worker enters its
            // bounded native call; shutdown must still wait for that worker.
            entered.recv_timeout(Duration::from_secs(3)).unwrap();
            drop(request);
        });
        ready_tx.send(()).unwrap();
        drop(executor);
        exited_tx.send(()).unwrap();
    });
    let admitted = ready.recv_timeout(Duration::from_secs(3));
    let premature_exit = exited.recv_timeout(Duration::from_millis(50)).is_ok();
    let held = runtime.try_begin_sync_run().is_none();
    let no_early_effect = effects.load(Ordering::SeqCst) == 0;
    let _ = release.send(());
    thread.join().unwrap();
    admitted.unwrap();
    assert!(
        !premature_exit,
        "native call cannot be preempted by async cancellation"
    );
    assert!(
        held,
        "runtime teardown must not release capacity before the native call"
    );
    assert!(no_early_effect);
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    assert_eq!(Arc::strong_count(&effects), 1);
    assert!(runtime.try_begin_sync_run().is_some());
}
