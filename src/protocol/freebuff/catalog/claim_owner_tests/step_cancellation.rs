//! Cancellation stays inside the owning future. Wire observation, not timers, triggers cooperative abort.
use super::completion::activate;
use super::multistep::intent;
use super::*;

#[tokio::test]
async fn freebuff_catalog_step_owner_cancel_before_prepare_has_no_model_attempt_or_spend() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_step(
                json!("not even a valid harness"),
                1,
                BUDGET,
                std::future::ready(()),
            )
            .await;
        assert_eq!(decoded.completion().err(), Some(BusinessError::Cancelled));
        assert_eq!(peer.count(), 2);
        let finished = decoded.finish_run(BUDGET).await.unwrap();
        assert_eq!(finished.totals.total_steps, 0);
        assert_eq!(finished.totals.total_credits, 0);
        let hits = peer.hits.lock().unwrap();
        let finish: Value = serde_json::from_slice(&hits[2].body).unwrap();
        assert_eq!(finish["status"], "cancelled");
    })
    .await;
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_step_owner_cancel_during_headers_keeps_unknown_run_and_claim() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let mut delayed = fixture::Spec::json(200, json!({}));
    delayed.header_delay = Duration::from_millis(150);
    peer.push(delayed);
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_step(intent(), 4096, BUDGET, async {
                while peer.count() < 3 {
                    tokio::task::yield_now().await;
                }
            })
            .await;
        assert_eq!(decoded.completion().err(), Some(BusinessError::Cancelled));
        let decoded = decoded.continue_run().err().unwrap();
        assert_eq!(
            decoded.finish_run(BUDGET).await.err(),
            Some(BusinessError::AccountingUnknown)
        );
        assert_eq!(
            peer.count(),
            3,
            "unknown send never becomes a zero-cost FINISH"
        );
    })
    .await;
    assert_eq!(row(&db).await, original);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

pub(super) fn stream(cost: f64) -> fixture::Spec {
    let mut spec = fixture::Spec::json(200, json!(null));
    spec.headers
        .insert("content-type", "text/event-stream".parse().unwrap());
    spec.body = format!(
        "data: {}\n\n",
        json!({"choices":[{"index":0,
        "delta":{"content":"answer"},"finish_reason":"stop"}],"usage":{"cost":cost}})
    )
    .into_bytes();
    spec.eof_delay = Duration::from_millis(600);
    spec
}

#[tokio::test]
async fn freebuff_catalog_step_owner_cancel_after_cost_preserves_report_and_requires_finish() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(stream(0.125));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let observed = Arc::new(tokio::sync::Notify::new());
        let decoded = TEST_BODY_STARTED
            .scope(
                observed.clone(),
                run.execute_step(intent(), 4096, BUDGET, observed.notified()),
            )
            .await;
        assert_eq!(decoded.completion().err(), Some(BusinessError::Cancelled));
        assert_eq!(peer.count(), 3, "cancellation does not implicitly finalize");
        let finished = decoded.finish_run(BUDGET).await.unwrap();
        assert_eq!(finished.totals.total_steps, 1);
        assert_eq!(finished.totals.total_credits, 13);
        let hits = peer.hits.lock().unwrap();
        let finish: Value = serde_json::from_slice(&hits[3].body).unwrap();
        assert_eq!(finish["status"], "cancelled");
        assert_eq!(finish["directCredits"], 13);
    })
    .await;
    peer.wait_drops(4).await;
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_step_owner_drop_after_send_has_no_detached_finish_or_replay() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(stream(0.125));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let observed = Arc::new(tokio::sync::Notify::new());
        TEST_BODY_STARTED
            .scope(observed.clone(), async {
                let mut future =
                    Box::pin(run.execute_step(intent(), 4096, BUDGET, std::future::pending()));
                tokio::select! {
                    _ = &mut future => panic!("fixture must still be awaiting real EOF"),
                    _ = observed.notified() => (),
                }
                // The parser observed a frame, so the server's tracked body really exists. A header-
                // phase handler may exit before constructing it and has no body Drop to count.
                drop(future);
                peer.wait_drops(3).await;
            })
            .await;
        assert_eq!(
            peer.count(),
            3,
            "Drop does not finish, retry, refund or release the session"
        );
    })
    .await;
    assert_eq!(row(&db).await, original);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
