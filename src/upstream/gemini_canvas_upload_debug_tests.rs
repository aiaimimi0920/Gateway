use super::*;
use std::time::Duration;

#[tokio::test]
async fn disabled_debug_does_not_execute_or_acquire_capacity() {
    let slots = Arc::new(Semaphore::new(1));
    // A successful builder would return Some; panic is intentionally absorbed by this API.
    let result = run_bounded_debug(false, &Bytes::new(), slots.clone(), |_| 17).await;
    assert!(result.is_none());
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn saturated_or_closed_debug_does_not_queue_work() {
    let slots = Arc::new(Semaphore::new(1));
    let permit = slots.clone().acquire_owned().await.unwrap();
    let result = run_bounded_debug(true, &Bytes::new(), slots.clone(), |_| 17).await;
    assert!(result.is_none());
    drop(permit);
    slots.close();
    let result = run_bounded_debug(true, &Bytes::new(), slots, |_| 17).await;
    assert!(result.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn debug_worker_shares_payload_and_runs_off_executor() {
    let slots = Arc::new(Semaphore::new(1));
    let bytes = Bytes::from(vec![42; 1024]);
    let pointer = bytes.as_ptr() as usize;
    let executor_thread = std::thread::current().id();
    let result = run_bounded_debug(true, &bytes, slots.clone(), move |owned| {
        assert_ne!(std::thread::current().id(), executor_thread);
        assert_eq!(owned.as_ptr() as usize, pointer);
        owned.len()
    })
    .await;
    assert_eq!(result, Some(1024));
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn debug_worker_panic_is_nonfatal_and_releases_admission() {
    let slots = Arc::new(Semaphore::new(1));
    let result: Option<()> = run_bounded_debug(true, &Bytes::new(), slots.clone(), |_| {
        panic!("synthetic diagnostic failure")
    })
    .await;
    assert!(result.is_none());
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_debug_caller_retains_capacity_until_worker_exits() {
    let slots = Arc::new(Semaphore::new(1));
    let worker_slots = slots.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let (finished, completion) = tokio::sync::oneshot::channel();
    let bytes = Bytes::from(vec![17; 1024]);
    let pointer = bytes.as_ptr() as usize;
    let caller = tokio::spawn(async move {
        run_bounded_debug(true, &bytes, worker_slots, move |owned| {
            started.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
            finished
                .send(owned.as_ptr() as usize == pointer && owned[0] == 17)
                .unwrap();
        })
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    // Reaching here while the worker blocks proves current-thread runtime progress.
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert_eq!(slots.available_permits(), 0);
    release.send(()).unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(5), completion)
        .await
        .unwrap()
        .unwrap());
    let permit = tokio::time::timeout(Duration::from_secs(5), slots.clone().acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert_eq!(slots.available_permits(), 1);
}
