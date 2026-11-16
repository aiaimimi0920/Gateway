//! The production step owner retains one run across prepare/send/decode, not a synthetic tool host.
use super::completion::activate;
use super::multistep::{intent, response};
use super::*;
fn sendable<T: Send>(future: T) -> T {
    future
}

#[tokio::test]
async fn freebuff_catalog_step_owner_two_steps_keep_original_claim_and_signed_wire() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(fixture::Spec::json(200, response(Some(0.125))));
    peer.push(fixture::Spec::json(200, response(Some(0.25))));
    peer.push(fixture::Spec::json(204, json!(null)));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded =
            sendable(run.execute_step(intent(), 4096, BUDGET, std::future::pending())).await;
        let (run, answer) = decoded.continue_run().unwrap();
        let mut next = intent();
        next["messages"].as_array_mut().unwrap().extend([
            answer["choices"][0]["message"].clone(),
            json!({"role":"user","content":"continue"}),
        ]);
        let finished = run
            .execute_step(next, 4096, BUDGET, std::future::pending())
            .await
            .finish_run(BUDGET)
            .await
            .unwrap();
        assert!(finished.completion.is_ok());
        assert_eq!(finished.totals.total_steps, 2);
        assert_eq!(finished.totals.total_credits, 39);
    })
    .await;
    assert_eq!(row(&db).await, original);
    {
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 5);
        let first: Value = serde_json::from_slice(&hits[2].body).unwrap();
        let second: Value = serde_json::from_slice(&hits[3].body).unwrap();
        assert_eq!(first["codebuff_metadata"], second["codebuff_metadata"]);
        assert_eq!(second["codebuff_metadata"]["run_id"], "original-run");
        assert_eq!(second["messages"][2]["content"], "continue");
        use base64::Engine;
        let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(device.key.public_key())
            .unwrap();
        business_tests::verify(&hits[2], &public, "synthetic-fetch");
        business_tests::verify(&hits[3], &public, "synthetic-fetch");
    }
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_step_owner_deadline_covers_headers_body_and_real_eof() {
    for phase in 0..3 {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        let mut spec = super::step_cancellation::stream(0.125);
        if phase == 0 {
            spec.header_delay = Duration::from_millis(600);
            spec.eof_delay = Duration::ZERO;
        }
        if phase == 1 {
            spec.body_delay = Duration::from_millis(600);
            spec.eof_delay = Duration::ZERO;
        }
        peer.push(spec);
        if phase == 2 {
            peer.push(fixture::Spec::json(204, json!(null)));
        }
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let decoded = run
                .execute_step(
                    intent(),
                    4096,
                    Duration::from_millis(200),
                    std::future::pending(),
                )
                .await;
            assert_eq!(decoded.completion().err(), Some(BusinessError::Timeout));
            assert_eq!(peer.count(), 3);
            let decoded = decoded.continue_run().err().unwrap();
            if phase == 2 {
                let finished = decoded.finish_run(BUDGET).await.unwrap();
                assert_eq!(finished.totals.total_steps, 1);
                assert_eq!(finished.totals.total_credits, 13);
                let hits = peer.hits.lock().unwrap();
                let finish: Value = serde_json::from_slice(&hits[3].body).unwrap();
                assert_eq!(finish["status"], "failed");
            } else {
                assert_eq!(
                    decoded.finish_run(BUDGET).await.err(),
                    Some(BusinessError::AccountingUnknown)
                );
                assert_eq!(peer.count(), 3);
            }
        })
        .await;
        assert_eq!(row(&db).await, original);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
