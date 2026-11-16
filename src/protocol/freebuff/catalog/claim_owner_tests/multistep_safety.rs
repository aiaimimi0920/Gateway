//! A failed/unknown step cannot be hidden by continuation, later zero or the old one-step API.
use super::completion::activate;
use super::multistep::{intent, response};
use super::*;
#[tokio::test]
async fn freebuff_catalog_multistep_unknown_in_any_step_blocks_final_finish() {
    for first_unknown in [true, false] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        peer.push(fixture::Spec::json(
            200,
            response(if first_unknown { None } else { Some(0.125) }),
        ));
        peer.push(fixture::Spec::json(
            200,
            response(if first_unknown { Some(0.0) } else { None }),
        ));
        let result = with_test_origin(peer.origin.clone(), async {
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
            run.prepare_completion(intent(), 4096)
                .unwrap()
                .send(BUDGET)
                .await
                .unwrap()
                .decode(BUDGET)
                .await
                .finish_run(BUDGET)
                .await
        })
        .await;
        assert_eq!(result.err(), Some(BusinessError::AccountingUnknown));
        assert_eq!(peer.count(), 4);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
#[tokio::test]
async fn freebuff_catalog_multistep_failed_second_step_preserves_first_cost_and_cannot_resume() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(fixture::Spec::json(200, response(Some(0.125))));
    peer.push(fixture::Spec::json(
        200,
        json!({"error":{"message":"synthetic-secret"},"usage":{"cost":0.25}}),
    ));
    peer.push(fixture::Spec::json(200, json!({})));
    let finished = with_test_origin(peer.origin.clone(), async {
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
        let decoded = run
            .prepare_completion(intent(), 4096)
            .unwrap()
            .send(BUDGET)
            .await
            .unwrap()
            .decode(BUDGET)
            .await;
        let failed = decoded
            .continue_run()
            .err()
            .expect("semantic failure must retain its owner");
        failed.finish_run(BUDGET).await.unwrap()
    })
    .await;
    assert!(finished.completion.is_err());
    assert_eq!(finished.totals.total_steps, 2);
    assert_eq!(finished.totals.total_credits, 39);
    {
        let hits = peer.hits.lock().unwrap();
        let finish: Value = serde_json::from_slice(&hits[4].body).unwrap();
        assert_eq!(finish["status"], "failed");
        assert_eq!(finish["directCredits"], 39);
        assert!(!String::from_utf8_lossy(&hits[4].body).contains("synthetic-secret"));
    }
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
#[tokio::test]
async fn freebuff_catalog_multistep_single_step_api_and_drop_never_truncate_or_auto_finish() {
    for single_step in [true, false] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        peer.push(fixture::Spec::json(200, response(Some(0.125))));
        if single_step {
            peer.push(fixture::Spec::json(200, response(Some(0.25))));
        }
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
            if single_step {
                let decoded = run
                    .prepare_completion(intent(), 4096)
                    .unwrap()
                    .send(BUDGET)
                    .await
                    .unwrap()
                    .decode(BUDGET)
                    .await;
                assert_eq!(
                    decoded.finish_single_step(BUDGET).await.err(),
                    Some(BusinessError::InvalidInput)
                );
            } else {
                drop(run);
            }
        })
        .await;
        assert_eq!(peer.count(), if single_step { 4 } else { 3 });
        assert_eq!(row(&db).await, original);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
