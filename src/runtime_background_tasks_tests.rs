use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::oneshot;

async fn held(tasks: &BackgroundTasks, kind: Worker) -> oneshot::Sender<()> {
    let (release, wait) = oneshot::channel();
    let (started, ready) = oneshot::channel();
    assert!(
        tasks.spawn(kind, &GatewayShutdownHandle::default(), async move {
            started.send(()).unwrap();
            wait.await.unwrap();
        })
    );
    ready.await.unwrap();
    release
}

#[tokio::test]
async fn duplicate_worker_keeps_original_handle_and_does_not_poll_rejected_future() {
    let tasks = BackgroundTasks::default();
    let release = held(&tasks, Worker::CredentialRefill).await;
    let observed = Arc::new(AtomicBool::new(false));
    let output = observed.clone();
    assert!(!tasks.spawn(
        Worker::CredentialRefill,
        &GatewayShutdownHandle::default(),
        async move {
            output.store(true, Ordering::Release);
        }
    ));
    assert_eq!(tasks.workers(), [Worker::CredentialRefill]);
    release.send(()).unwrap();
    tasks.join().await.unwrap();
    assert!(!observed.load(Ordering::Acquire));
}

#[tokio::test]
async fn completed_worker_can_restart_before_close() {
    let tasks = BackgroundTasks::default();
    let (done, wait) = oneshot::channel();
    assert!(tasks.spawn(
        Worker::NetworkProbe,
        &GatewayShutdownHandle::default(),
        async move {
            done.send(()).unwrap();
        }
    ));
    wait.await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !tasks.spawn(
            Worker::NetworkProbe,
            &GatewayShutdownHandle::default(),
            async {},
        ) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tasks.join().await.unwrap();
}

#[tokio::test]
async fn requested_stop_and_sealed_owner_reject_new_workers() {
    let tasks = BackgroundTasks::default();
    let stop = GatewayShutdownHandle::default();
    stop.request("fixture stop");
    assert!(!tasks.spawn(Worker::Discovery, &stop, async { panic!("must not start") }));
    tasks.join().await.unwrap();
    assert!(!tasks.spawn(
        Worker::Discovery,
        &GatewayShutdownHandle::default(),
        async { panic!("must not reopen") }
    ));
}

#[tokio::test]
async fn cancelled_join_waiter_keeps_admitted_worker_until_completion() {
    let tasks = BackgroundTasks::default();
    let release = held(&tasks, Worker::FolderSync).await;
    let mut join = Box::pin(tasks.join());
    assert!(join.as_mut().now_or_never().is_none());
    drop(join);
    assert_eq!(tasks.workers(), [Worker::FolderSync]);
    release.send(()).unwrap();
    tasks.join().await.unwrap();
    assert!(tasks.workers().is_empty());
}

#[tokio::test]
async fn concurrent_joiner_can_continue_after_first_waiter_is_cancelled() {
    let tasks = BackgroundTasks::default();
    let release = held(&tasks, Worker::PoolAutomation).await;
    let mut first = Box::pin(tasks.join());
    let mut second = Box::pin(tasks.join());
    assert!(first.as_mut().now_or_never().is_none());
    assert!(second.as_mut().now_or_never().is_none());
    drop(first);
    release.send(()).unwrap();
    second.await.unwrap();
    tasks.join().await.unwrap();
}

#[tokio::test]
async fn panic_is_retained_after_waiter_cancellation_and_does_not_skip_other_workers() {
    let tasks = BackgroundTasks::default();
    let release = held(&tasks, Worker::CredentialRefresh).await;
    assert!(tasks.spawn(
        Worker::Discovery,
        &GatewayShutdownHandle::default(),
        async { panic!("fixture hidden panic payload") }
    ));
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !tasks.state.lock().tasks[&Worker::Discovery].is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let mut join = Box::pin(tasks.join());
    assert!(join.as_mut().now_or_never().is_none());
    drop(join);
    release.send(()).unwrap();
    let error = tasks.join().await.unwrap_err().to_string();
    assert!(error.contains("Discovery: panicked"));
    assert!(!error.contains("fixture hidden panic payload"));
    assert_eq!(tasks.join().await.unwrap_err().to_string(), error);
    assert!(tasks.workers().is_empty());
}

#[tokio::test]
async fn workers_and_close_state_are_scoped_to_one_runtime() {
    let first = BackgroundTasks::default();
    let second = BackgroundTasks::default();
    first.join().await.unwrap();
    let release = held(&second, Worker::LocalMaintenance).await;
    release.send(()).unwrap();
    second.join().await.unwrap();
}
