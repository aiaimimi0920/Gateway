use super::*;

fn test_worker() -> Arc<ManagedWorker> {
    Arc::new(ManagedWorker {
        id: "worker-test".to_string(),
        port: 4201,
        base_url: "http://127.0.0.1:4201".to_string(),
        executable_path: "gateway".to_string(),
        started_at: "test".to_string(),
        status: RwLock::new(SplitterWorkerStatus::Active),
        drain_requested_at: RwLock::new(None),
        drain_reason: RwLock::new(None),
        exit_status: RwLock::new(None),
        child: Mutex::new(None),
        active_requests: AtomicUsize::new(0),
        request_notify: tokio::sync::Notify::new(),
        pid: None,
    })
}

#[tokio::test]
async fn worker_lease_admission_closes_on_drain_and_waits_for_release() {
    let worker = test_worker();
    let lease = worker
        .try_acquire_lease()
        .expect("active worker must admit a request");
    assert_eq!(worker.active_requests(), 1);

    assert!(worker.transition_to(SplitterWorkerStatus::Draining).is_ok());
    assert!(worker.try_acquire_lease().is_none());

    let wait = worker.wait_for_no_active_requests(Duration::from_secs(1));
    tokio::pin!(wait);
    assert!(tokio::time::timeout(Duration::from_millis(20), &mut wait)
        .await
        .is_err());

    drop(lease);
    assert!(wait.await);
    assert_eq!(worker.active_requests(), 0);
}

#[tokio::test]
async fn splitter_readiness_probe_timeout_is_bounded() {
    let started = tokio::time::Instant::now();
    let result = tokio::time::timeout(Duration::from_millis(20), async {
        std::future::pending::<bool>().await
    })
    .await;

    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(splitter_readiness_endpoint_timeout() <= Duration::from_secs(5));
}
