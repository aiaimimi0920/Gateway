//! Recovery reads facts, but even a fresh heartbeat cannot replay an existing START intent.
use super::*;
async fn journal(db: &LocalRuntime, session: &CatalogSession) -> Value {
    serde_json::to_value(
        db.freebuff_claim_journal("pool", "account", &session.stored_claim_scope(), None)
            .await
            .unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn freebuff_catalog_run_journal_reopen_and_heartbeat_never_replay_original_start() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    with_test_origin(peer.origin.clone(), async {
        drop(owner.start_catalog_run(&requests, BUDGET).await.unwrap());
    })
    .await;
    let before = journal(&db, &session).await;
    assert_eq!(before["claim"]["run"]["phase"], "started");
    drop(owner);
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(journal(&db, &session).await, before);
    let scope = requests.recovery_requests();
    let saved = recovery(&db, &scope).await;
    peer.push(fixture::Spec::json(200, active(saved.claim(), "open")));
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
        3,
        "heartbeat is not a second START permission"
    );
    assert_eq!(row(&db).await, original);
    assert_eq!(journal(&db, &session).await, before);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
#[tokio::test]
async fn freebuff_catalog_run_journal_keeps_rounded_multistep_cost_and_rejects_tampering() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    for cost in [0.125, 0.25] {
        peer.push(fixture::Spec::json(200, multistep::response(Some(cost))));
    }
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let (run, _) = run
            .execute_step(multistep::intent(), 4096, BUDGET, std::future::pending())
            .await
            .continue_run()
            .unwrap();
        let first = journal(&db, &session).await;
        assert_eq!(first["claim"]["run"]["totals"]["totalCredits"], 13);
        run.execute_step(multistep::intent(), 4096, BUDGET, std::future::pending())
            .await
            .finish_run(BUDGET)
            .await
            .unwrap();
    })
    .await;
    let recorded = journal(&db, &session).await;
    assert_eq!(recorded["claim"]["run"]["totals"]["totalSteps"], 2);
    assert_eq!(recorded["claim"]["run"]["totals"]["totalCredits"], 39);
    assert_eq!(recorded["claim"]["run"]["lastStep"]["cost"], 0.25);
    sqlx::query("UPDATE local_freebuff_runs SET sealed='corrupt'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(db
        .freebuff_claim_journal("pool", "account", &session.stored_claim_scope(), None)
        .await
        .is_err());
    assert_eq!(peer.count(), 5, "reading corrupt facts sends nothing");
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_run_journal_sqlite_wait_never_extends_active_observation() {
    for phase in ["start", "model"] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let prepared = pending(&db, &requests).await;
        let claim = prepared.claim().clone();
        drop(prepared);
        let mut live = active(&claim, "open");
        live["remainingMs"] = json!(1000);
        peer.push(fixture::Spec::json(200, live));
        let scope = requests.recovery_requests();
        let mut owner = with_test_origin(
            peer.origin.clone(),
            recovery(&db, &scope).await.heartbeat(&scope, BUDGET),
        )
        .await
        .unwrap();
        if phase == "start" {
            let writer = db.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
            with_test_origin(peer.origin.clone(), async {
                let start = owner.start_catalog_run(&requests, BUDGET);
                tokio::pin!(start);
                tokio::select! { _ = &mut start => panic!("START must wait for the real SQLite writer"), _ = tokio::time::sleep(Duration::from_millis(1200)) => {} }
                writer.commit().await.unwrap();
                assert_eq!(start.await.err(), Some(BusinessError::SessionInactive));
            }).await;
        } else {
            peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
            with_test_origin(peer.origin.clone(), async {
                let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
                let writer = db.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
                let step = run.execute_step(multistep::intent(), 4096, BUDGET, std::future::pending());
                tokio::pin!(step);
                tokio::select! { _ = &mut step => panic!("model send must wait for the real SQLite writer"), _ = tokio::time::sleep(Duration::from_millis(1200)) => {} }
                writer.commit().await.unwrap();
                assert_eq!(step.await.completion().err(), Some(BusinessError::SessionInactive));
            }).await;
        }
        assert_eq!(
            peer.count(),
            if phase == "start" { 1 } else { 2 },
            "expired observation must send no new START/model"
        );
        let facts = journal(&db, &session).await;
        assert_eq!(facts["claim"]["run"]["totals"]["totalCredits"], 0);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_catalog_run_journal_unknown_next_step_retains_known_prefix_after_drop() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(fixture::Spec::json(200, multistep::response(Some(0.125))));
    let mut pending = fixture::Spec::json(200, multistep::response(Some(0.25)));
    pending.header_delay = Duration::from_secs(1);
    peer.push(pending);
    let before = with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let (run, _) = run.execute_step(multistep::intent(), 4096, BUDGET, std::future::pending()).await.continue_run().unwrap();
        let step = run.execute_step(multistep::intent(), 4096, BUDGET, std::future::pending());
        tokio::pin!(step);
        tokio::select! {
            _ = &mut step => panic!("second response is deliberately pending"),
            facts = tokio::time::timeout(Duration::from_millis(800), async {
                loop {
                    let facts = journal(&db, &session).await;
                    if peer.count() == 4 && facts["claim"]["run"]["phase"] == "model_pending" { break facts; }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }) => facts.unwrap(),
        }
    }).await;
    assert!(before["claim"]["run"]["totals"].is_null());
    assert_eq!(before["claim"]["run"]["priorTotals"]["totalSteps"], 1);
    assert_eq!(before["claim"]["run"]["priorTotals"]["totalCredits"], 13);
    drop(owner);
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(journal(&db, &session).await, before);
    assert_eq!(
        peer.count(),
        4,
        "no guessed second cost, FINISH or replay after Drop/reopen"
    );
    peer.close().await;
    cleanup(&root, db).await;
}
