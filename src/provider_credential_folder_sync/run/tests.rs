use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;

use tokio::sync::oneshot;

use super::owner::run_owned;
use crate::error::GatewayError;
use crate::state::ProviderCredentialFolderSyncRuntime;

mod blocking;

async fn wait_for_run_release(runtime: &ProviderCredentialFolderSyncRuntime) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while runtime.try_begin_sync_run().is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("owned run releases admission after its final effect");
}

#[tokio::test]
async fn busy_runtime_clones_do_not_invoke_input_factory() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let cloned = runtime.clone();
    let (release, wait) = oneshot::channel();
    let mut first = Box::pin(run_owned(&runtime, || async move {
        wait.await.unwrap();
        Ok(7)
    }));
    assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
    let prepared = AtomicUsize::new(0);
    let error = run_owned(&cloned, || {
        prepared.fetch_add(1, Ordering::SeqCst);
        async { Ok(9) }
    })
    .await
    .unwrap_err();
    assert_eq!(prepared.load(Ordering::SeqCst), 0);
    assert_eq!(error.http_status, Some(409));
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_run_busy")
    );
    release.send(()).unwrap();
    assert_eq!(first.await.unwrap(), 7);
    assert_eq!(run_owned(&cloned, || async { Ok(9) }).await.unwrap(), 9);
}

#[tokio::test]
async fn cancelled_caller_retains_capacity_through_late_effects() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let effects = Arc::new(AtomicUsize::new(0));
    let owned_effects = effects.clone();
    let (release, wait) = oneshot::channel();
    let (finished, done) = oneshot::channel();
    let mut first = Box::pin(run_owned(&runtime, || async move {
        wait.await.unwrap();
        owned_effects.fetch_add(1, Ordering::SeqCst);
        finished.send(()).unwrap();
        Ok(())
    }));
    assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
    drop(first);
    runtime.set_enabled(false);
    assert!(runtime.try_begin_sync_run().is_none());
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    release.send(()).unwrap();
    done.await.unwrap();
    // A worker can signal before releasing its last resources on another thread.
    wait_for_run_release(&runtime).await;
    assert!(runtime.try_begin_sync_run().is_some());
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    assert_eq!(Arc::strong_count(&effects), 1);
}

#[tokio::test]
async fn operation_errors_release_admission_and_preserve_error() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let error = run_owned::<(), _, _>(&runtime, || async {
        Err(GatewayError::bad_request("fixture operation error"))
    })
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.message, "fixture operation error");
    assert_eq!(run_owned(&runtime, || async { Ok(42) }).await.unwrap(), 42);
}

#[tokio::test]
async fn operation_panics_release_admission_and_map_to_fixed_error() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let error = run_owned::<(), _, _>(&runtime, || async {
        panic!("fixture task panic");
    })
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_run_task_failed")
    );
    assert_eq!(error.message, "folder sync run operation task failed");
    assert_eq!(run_owned(&runtime, || async { Ok(42) }).await.unwrap(), 42);
}

#[tokio::test]
async fn separate_runtimes_and_enable_updates_remain_independent() {
    let first = ProviderCredentialFolderSyncRuntime::new(true);
    let second = ProviderCredentialFolderSyncRuntime::new(false);
    let _held = first.try_begin_sync_run().unwrap();
    let mut enable = Box::pin(first.begin_enable_update());
    assert!(matches!(futures::poll!(enable.as_mut()), Poll::Ready(_)));
    assert_eq!(run_owned(&second, || async { Ok(42) }).await.unwrap(), 42);
    assert!(first.try_begin_sync_run().is_none());
}
