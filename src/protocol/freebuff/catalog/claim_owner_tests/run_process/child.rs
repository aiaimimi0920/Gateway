//! Explicit ignored child entry: only synthetic loopback and a parent-created temporary root.
use super::*;

#[tokio::test]
#[ignore = "parent-owned crash fixture; run only with the isolated child environment"]
async fn freebuff_run_process_child() {
    let root = PathBuf::from(std::env::var_os(CHILD_ROOT).expect("parent root"));
    let root = root.canonicalize().unwrap();
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("gateway-local-freebuff-claim-"));
    let origin = std::env::var(CHILD_ORIGIN).unwrap();
    let parsed = url::Url::parse(&origin).unwrap();
    assert_eq!(parsed.scheme(), "http");
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert!(parsed.port().is_some());
    let phase = std::env::var(CHILD_PHASE).unwrap();
    assert!([
        "start_pending",
        "started",
        "model_pending",
        "model_observed",
        "finish_pending",
        "finish_accepted"
    ]
    .contains(&phase.as_str()));
    let db = LocalRuntime::open(&root).await.unwrap();
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    let identity = json!({"attemptId":prepared.claim().attempt_id(), "instanceId":prepared.claim().instance_id()});
    tokio::fs::write(
        root.join("process-ready.json"),
        serde_json::to_vec(&identity).unwrap(),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        while !root.join("process-armed").is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    with_test_origin(origin, async {
        let budget = Duration::from_secs(60);
        let mut owner = prepared.send(budget).await.unwrap();
        let run = owner.start_catalog_run(&requests, budget).await.unwrap();
        if phase == "started" {
            // Keep the actual owner and its OS fence alive until the parent kills this process.
            std::future::pending::<()>().await;
        }
        run.execute_step(multistep::intent(), 4096, budget, std::future::pending())
            .await
            .finish_run(budget)
            .await
            .unwrap();
        assert_eq!(
            phase, "finish_accepted",
            "a deliberately stalled phase unexpectedly completed"
        );
        std::future::pending::<()>().await;
    })
    .await;
}
