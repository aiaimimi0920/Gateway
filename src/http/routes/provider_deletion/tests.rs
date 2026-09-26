use super::run_owned;
use crate::error::GatewayError;
use crate::state::GatewayLifecycleState;
use std::sync::{mpsc, Arc};
use std::task::Poll;
use std::time::Duration;
use tokio::sync::oneshot;

mod lifetime;

#[test]
fn dedicated_owner_does_not_wait_for_shared_blocking_pool() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let lifecycle = GatewayLifecycleState::default();
    runtime.block_on(async {
        let (occupied, ready) = oneshot::channel();
        let (release, wait) = mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            occupied.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(3)).unwrap();
        });
        ready.await.unwrap();
        let (started_tx, started) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let mut operation = Box::pin(run_owned(&lifecycle, || async move {
            started_tx.send(()).unwrap();
            finish_rx.await.unwrap();
            Ok(())
        }));
        assert!(matches!(futures::poll!(operation.as_mut()), Poll::Pending));
        tokio::time::timeout(Duration::from_millis(250), started)
            .await
            .expect("dedicated owner must start while shared pool is occupied")
            .unwrap();
        assert!(lifecycle.try_begin_provider_deletion().is_none());
        drop(operation);
        finish_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while lifecycle.try_begin_provider_deletion().is_none() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("dedicated owner must release deletion admission after completion");
        release.send(()).unwrap();
        blocker.await.unwrap();
    });
}

#[tokio::test]
async fn native_work_does_not_block_the_executor() {
    let lifecycle = GatewayLifecycleState::default();
    let caller = std::thread::current().id();
    let (release, wait) = mpsc::channel();
    let heartbeat = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = release.send(());
    });
    let result = run_owned(&lifecycle, || async move {
        let responsive = wait.recv_timeout(Duration::from_secs(1)).is_ok();
        Ok((responsive, std::thread::current().id()))
    })
    .await
    .unwrap();
    heartbeat.await.unwrap();
    assert!(result.0, "native work blocked the executor heartbeat");
    assert_ne!(result.1, caller);
}

#[tokio::test]
async fn caller_drop_keeps_ordered_effects_and_admission() {
    let lifecycle = Arc::new(GatewayLifecycleState::default());
    let (started, ready) = oneshot::channel();
    let (release, wait) = oneshot::channel();
    let (finished, done) = oneshot::channel();
    let state = Arc::clone(&lifecycle);
    let caller = tokio::spawn(async move {
        run_owned(&state, || async move {
            let mut effects = vec!["file"];
            let _ = started.send(());
            let _ = wait.await;
            effects.push("database");
            effects.push("redis");
            let _ = finished.send(effects);
            Ok(())
        })
        .await
    });
    ready.await.unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    let rejected = run_owned(&lifecycle, || async { Ok(9) }).await;
    let _ = release.send(());
    let effects = tokio::time::timeout(Duration::from_secs(3), done).await;
    assert_eq!(effects.unwrap().unwrap(), ["file", "database", "redis"]);
    let error = rejected.unwrap_err();
    assert_eq!(error.http_status, Some(409));
    assert_eq!(
        error.code.as_deref(),
        Some("provider_management_delete_busy")
    );
}

#[tokio::test]
async fn busy_rejects_before_preparation_and_other_states_are_independent() {
    let lifecycle = Arc::new(GatewayLifecycleState::default());
    let (started, ready) = oneshot::channel();
    let (release, wait) = oneshot::channel();
    let state = Arc::clone(&lifecycle);
    let caller = tokio::spawn(async move {
        run_owned(&state, || async move {
            let _ = started.send(());
            let _ = wait.await;
            Ok(())
        })
        .await
    });
    ready.await.unwrap();
    let mut prepared = false;
    let result = run_owned(&lifecycle, || {
        prepared = true;
        async { Ok(()) }
    })
    .await;
    let independent = run_owned(&GatewayLifecycleState::default(), || async { Ok(7) }).await;
    let _ = release.send(());
    caller.await.unwrap().unwrap();
    assert!(!prepared);
    assert_eq!(result.unwrap_err().http_status, Some(409));
    assert_eq!(independent.unwrap(), 7);
    run_owned(&lifecycle, || async { Ok(()) }).await.unwrap();
}

#[tokio::test]
async fn operation_errors_are_preserved_and_release_capacity() {
    let lifecycle = GatewayLifecycleState::default();
    let error = run_owned::<(), _, _>(&lifecycle, || async {
        let mut error = GatewayError::bad_request("synthetic failure");
        error.code = Some("synthetic_code".into());
        Err(error)
    })
    .await
    .unwrap_err();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("synthetic_code"));
    assert_eq!(error.message, "synthetic failure");
    run_owned(&lifecycle, || async { Ok(()) }).await.unwrap();
}

#[tokio::test]
async fn panic_has_fixed_diagnostic_and_releases_capacity() {
    let lifecycle = Arc::new(GatewayLifecycleState::default());
    let state = Arc::clone(&lifecycle);
    let caller = tokio::spawn(async move {
        run_owned::<(), _, _>(&state, || async { panic!("synthetic private detail") }).await
    });
    let result = caller.await;
    run_owned(&lifecycle, || async { Ok(()) }).await.unwrap();
    let error = result.unwrap().unwrap_err();
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("provider_management_delete_task_failed")
    );
    assert!(!error.message.contains("private detail"));
}
