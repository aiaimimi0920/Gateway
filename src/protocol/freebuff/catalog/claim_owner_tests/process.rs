//! Actual child-process crash releases only the OS fence, never the prepare-before-send record.
use super::*;
const CHILD_ROOT: &str = "GATEWAY_TEST_FREEBUFF_CLAIM_CHILD_ROOT";
#[tokio::test]
async fn freebuff_claim_process_writer() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(root.starts_with(std::env::temp_dir()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("gateway-local-freebuff-claim-"));
    let db = LocalRuntime::open(&root).await.unwrap();
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    tokio::fs::write(root.join("child-ready.txt"), prepared.claim().attempt_id())
        .await
        .unwrap();
    std::future::pending::<()>().await;
    drop(prepared);
}

#[tokio::test]
async fn freebuff_claim_actual_process_crash_keeps_identity_and_releases_fence() {
    let (root, db) = setup().await;
    let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact").arg("protocol::freebuff::catalog::claim_owner_tests::process::freebuff_claim_process_writer")
        .arg("--nocapture").env(CHILD_ROOT, &root).kill_on_drop(true)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let ready = root.join("child-ready.txt");
    let id = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Ok(id) = tokio::fs::read_to_string(&ready).await {
                if id.len() == 36 {
                    break id;
                }
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "fixture child exited before persisting its claim"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    assert_eq!(
        db.prepare_freebuff_admission(
            "pool",
            "account",
            &requests,
            SessionChargeConsent::MaySpendFreebucks
        )
        .await
        .err()
        .unwrap()
        .code
        .as_deref(),
        Some("freebuff_session_claim_busy")
    );
    child.kill().await.unwrap();
    assert!(child.wait().await.unwrap().code() != Some(0));
    let requests = session.recovery_requests(&device).unwrap();
    let recovered = recovery(&db, &requests).await;
    assert_eq!(recovered.claim().attempt_id(), id);
    assert_eq!(row(&db).await.1, id);
    drop(recovered);
    cleanup(&root, db).await;
}
