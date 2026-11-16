//! Cooperative cancellation preserves observed costs and original ownership; Drop never finalizes.
use super::completion::activate;
use super::multistep::{intent, response};
use super::*;

#[tokio::test]
async fn freebuff_catalog_multistep_cancel_during_body_keeps_partial_cost_and_requires_explicit_finish(
) {
    for known in [true, false] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        peer.push(fixture::Spec::json(200, response(Some(0.125))));
        let mut second = fixture::Spec::json(200, json!(null));
        second
            .headers
            .insert("content-type", "text/event-stream".parse().unwrap());
        second.body = format!(
            "data: {}\n\n",
            json!({"choices":[{"index":0,"delta":{"content":"partial"}}],
            "usage":{"cost":if known {Some(0.25)}else{None}}})
        )
        .into_bytes();
        second.eof_delay = Duration::from_secs(10);
        peer.push(second);
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let decoded = run
                .prepare_completion(intent(), 4096)
                .unwrap()
                .send(BUDGET)
                .await
                .unwrap()
                .decode(BUDGET)
                .await;
            let (run, _) = decoded.continue_run().unwrap();
            let response = run
                .prepare_completion(intent(), 4096)
                .unwrap()
                .send(BUDGET)
                .await
                .unwrap();
            let observed = Arc::new(tokio::sync::Notify::new());
            let decoded = TEST_BODY_STARTED
                .scope(
                    observed.clone(),
                    response.decode_until(BUDGET, observed.notified()),
                )
                .await;
            assert_eq!(decoded.completion().err(), Some(BusinessError::Cancelled));
            let decoded = decoded
                .continue_run()
                .err()
                .expect("cancelled step must not resume");
            assert_eq!(
                peer.count(),
                4,
                "cancellation does not implicitly send FINISH"
            );
            peer.wait_drops(4).await;
            if known {
                peer.push(fixture::Spec::json(204, json!(null)));
                let finished = decoded.finish_run(BUDGET).await.unwrap();
                assert_eq!(finished.totals.total_steps, 2);
                assert_eq!(finished.totals.direct_credits, 39);
                assert_eq!(finished.completion.err(), Some(BusinessError::Cancelled));
                let hits = peer.hits.lock().unwrap();
                let finish: Value = serde_json::from_slice(&hits[4].body).unwrap();
                assert_eq!(finish["status"], "cancelled");
                assert_eq!(finish["totalSteps"], 2);
                assert_eq!(finish["directCredits"], 39);
                assert_eq!(finish["totalCredits"], 39);
                assert_eq!(finish["runId"], "original-run");
            } else {
                assert_eq!(
                    decoded.finish_run(BUDGET).await.err(),
                    Some(BusinessError::AccountingUnknown)
                );
                assert_eq!(
                    peer.count(),
                    4,
                    "unknown in-flight cost cannot become prior known spend or zero"
                );
            }
        })
        .await;
        assert_eq!(row(&db).await, original);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
#[tokio::test]
async fn freebuff_catalog_multistep_cancel_between_steps_or_before_first_keeps_exact_totals() {
    for stepped in [true, false] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        if stepped {
            peer.push(fixture::Spec::json(200, response(Some(0.125))));
        }
        peer.push(fixture::Spec::json(204, json!(null)));
        with_test_origin(peer.origin.clone(), async {
            let mut run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            if stepped {
                let decoded = run
                    .prepare_completion(intent(), 4096)
                    .unwrap()
                    .send(BUDGET)
                    .await
                    .unwrap()
                    .decode(BUDGET)
                    .await;
                run = decoded.continue_run().unwrap().0;
            }
            let other = LocalRuntime::open(&root).await.unwrap();
            assert_eq!(
                other
                    .recover_freebuff_admission("pool", "account", &requests.recovery_requests())
                    .await
                    .err()
                    .unwrap()
                    .code
                    .as_deref(),
                Some("freebuff_session_claim_busy")
            );
            let finished = run.finish_cancelled(BUDGET).await.unwrap();
            let steps = u64::from(stepped);
            let credits = if stepped { 13 } else { 0 };
            assert_eq!(finished.totals.total_steps, steps);
            assert_eq!(finished.totals.total_credits, credits);
            let hits = peer.hits.lock().unwrap();
            let finish: Value = serde_json::from_slice(&hits.last().unwrap().body).unwrap();
            assert_eq!(finish["status"], "cancelled");
            assert_eq!(finish["totalSteps"], steps);
            assert_eq!(finish["totalCredits"], credits);
            drop(hits);
            other.close().await;
        })
        .await;
        assert_eq!(row(&db).await, original);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
