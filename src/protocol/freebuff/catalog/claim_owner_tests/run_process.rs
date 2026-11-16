//! Real process death at each durable run boundary must release the fence, never replay the wire.
use super::*;
mod child;
const CHILD_ROOT: &str = "GATEWAY_TEST_FREEBUFF_RUN_CHILD_ROOT";
const CHILD_ORIGIN: &str = "GATEWAY_TEST_FREEBUFF_RUN_CHILD_ORIGIN";
const CHILD_PHASE: &str = "GATEWAY_TEST_FREEBUFF_RUN_CHILD_PHASE";

async fn facts(db: &LocalRuntime, session: &CatalogSession) -> Value {
    serde_json::to_value(
        db.freebuff_claim_journal("pool", "account", &session.stored_claim_scope(), None)
            .await
            .unwrap(),
    )
    .unwrap()
}
async fn run_row(db: &LocalRuntime) -> (String, String, String, String) {
    sqlx::query_as("SELECT binding,attempt_id,claim_sealed,sealed FROM local_freebuff_runs")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}
fn active_reply(identity: &Value) -> Value {
    let now = OffsetDateTime::now_utc();
    json!({"status":"active", "instanceId":identity["instanceId"], "model":"open", "accessTier":"full",
        "admittedAt":(now-time::Duration::seconds(1)).format(&time::format_description::well_known::Rfc3339).unwrap(),
        "expiresAt":(now+time::Duration::hours(1)).format(&time::format_description::well_known::Rfc3339).unwrap(), "remainingMs":3600000})
}
fn queue(peer: &fixture::Peer, identity: &Value, phase: &str) -> usize {
    peer.push(fixture::Spec::json(200, active_reply(identity)));
    let mut start = fixture::Spec::json(200, json!({"runId":"original-process-run"}));
    if phase == "start_pending" {
        start.body_delay = Duration::from_secs(30);
    }
    peer.push(start);
    if matches!(phase, "start_pending" | "started") {
        return 2;
    }
    let mut model = fixture::Spec::json(200, multistep::response(Some(0.125)));
    if phase == "model_pending" {
        model.body_delay = Duration::from_secs(30);
    } else if phase == "model_observed" {
        model
            .headers
            .insert("content-type", "text/event-stream".parse().unwrap());
        model.body = format!("data: {}\n\n", json!({"id":"answer","model":"reported-model",
            "choices":[{"index":0,"delta":{"role":"assistant","content":"answer"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7,"cost":0.125}})).into_bytes();
        model.eof_delay = Duration::from_secs(30);
    }
    peer.push(model);
    if matches!(phase, "model_pending" | "model_observed") {
        return 3;
    }
    let mut finish = fixture::Spec::json(200, json!({}));
    if phase == "finish_pending" {
        finish.body_delay = Duration::from_secs(30);
    }
    peer.push(finish);
    4
}
async fn ready(root: &Path, child: &mut tokio::process::Child) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Ok(bytes) = tokio::fs::read(root.join("process-ready.json")).await {
                if let Ok(identity) = serde_json::from_slice::<Value>(&bytes) {
                    break identity;
                }
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "child exited before claim commit"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
async fn checkpoint(
    db: &LocalRuntime,
    session: &CatalogSession,
    peer: &fixture::Peer,
    child: &mut tokio::process::Child,
    phase: &str,
    sends: usize,
) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let view = facts(db, session).await;
            if view["claim"]["run"]["phase"] == phase && peer.count() == sends {
                break view;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "child exited before {phase}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn freebuff_catalog_run_actual_process_crash_preserves_six_phases_without_replay() {
    for phase in [
        "start_pending",
        "started",
        "model_pending",
        "model_observed",
        "finish_pending",
        "finish_accepted",
    ] {
        let (root, db) = setup().await;
        let peer = fixture::Peer::start(vec![]).await;
        let output = std::fs::File::create(root.join("child-output.log")).unwrap();
        let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
            .arg("--ignored").arg("--exact")
            .arg("protocol::freebuff::catalog::claim_owner_tests::run_process::child::freebuff_run_process_child")
            .arg("--nocapture").env(CHILD_ROOT, &root).env(CHILD_ORIGIN, &peer.origin).env(CHILD_PHASE, phase)
            .kill_on_drop(true).stdin(std::process::Stdio::null())
            .stdout(output.try_clone().unwrap()).stderr(output).spawn().unwrap();
        let pid = child.id().unwrap();
        let identity = ready(&root, &mut child).await;
        let attempt = identity["attemptId"].as_str().unwrap();
        assert_eq!(identity["instanceId"], format!("cli:{attempt}"));
        let sends = queue(&peer, &identity, phase);
        tokio::fs::write(root.join("process-armed"), b"synthetic-only")
            .await
            .unwrap();
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let before = checkpoint(&db, &session, &peer, &mut child, phase, sends).await;
        assert_eq!(before["claim"]["attemptId"], attempt);
        assert_eq!(
            before["claim"]["run"]["runIdRecorded"],
            phase != "start_pending"
        );
        assert_eq!(
            before["claim"]["run"]["totals"]["totalCredits"],
            match phase {
                "model_pending" => Value::Null,
                "start_pending" | "started" => json!(0),
                _ => json!(13),
            }
        );
        let original = row(&db).await;
        let run = run_row(&db).await;
        let seal = std::fs::read(root.join("access-key-seal.json")).unwrap();
        assert_eq!(
            db.recover_freebuff_admission("pool", "account", &requests.recovery_requests())
                .await
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("freebuff_session_claim_busy")
        );
        child.kill().await.unwrap();
        assert!(child.wait().await.unwrap().code() != Some(0));
        peer.wait_drops(sends).await;
        db.close().await;
        let db = LocalRuntime::open(&root).await.unwrap();
        assert_eq!(facts(&db, &session).await, before);
        assert_eq!(row(&db).await, original);
        assert_eq!(run_row(&db).await, run);
        assert_eq!(
            std::fs::read(root.join("access-key-seal.json")).unwrap(),
            seal
        );
        let scope = requests.recovery_requests();
        let saved = recovery(&db, &scope).await;
        assert_eq!(saved.claim().attempt_id(), attempt);
        peer.push(fixture::Spec::json(200, active_reply(&identity)));
        let mut owner = with_test_origin(peer.origin.clone(), saved.heartbeat(&scope, BUDGET))
            .await
            .unwrap();
        assert_eq!(
            with_test_origin(
                peer.origin.clone(),
                owner.start_catalog_run(&requests, BUDGET)
            )
            .await
            .err(),
            Some(BusinessError::RunJournalUnavailable)
        );
        assert_eq!(
            peer.count(),
            sends + 1,
            "only explicit original heartbeat, never START/FINISH/end/rebuy"
        );
        assert_eq!(facts(&db, &session).await, before);
        assert_eq!(row(&db).await, original);
        assert_eq!(run_row(&db).await, run);
        println!("process crash verified: phase={phase} pid={pid} original_attempt={attempt} sends={sends}");
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
