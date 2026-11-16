//! Definite local rejection preserves prior totals; possible sends keep unknown obligations, not zero.
use super::completion::activate;
use super::multistep::{intent, response};
use super::*;

#[tokio::test]
async fn freebuff_catalog_step_owner_prepare_errors_keep_prior_cost_without_another_attempt() {
    for kind in 0..4 {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        peer.push(fixture::Spec::json(200, response(Some(0.125))));
        peer.push(fixture::Spec::json(204, json!(null)));
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let decoded = run
                .execute_step(intent(), 4096, BUDGET, std::future::pending())
                .await;
            let (run, _) = decoded.continue_run().unwrap();
            let mut body = intent();
            let (limit, budget, error) = match kind {
                0 => {
                    body["model"] = json!("foreign-handle");
                    (4096, BUDGET, BusinessError::InvalidInput)
                }
                1 => (2, BUDGET, BusinessError::RequestTooLarge),
                2 => (4096, Duration::ZERO, BusinessError::InvalidInput),
                _ => {
                    body["codebuff_metadata"] = json!("synthetic-secret");
                    (4096, BUDGET, BusinessError::InvalidInput)
                }
            };
            let decoded = run
                .execute_step(body, limit, budget, std::future::pending())
                .await;
            assert_eq!(decoded.completion().err(), Some(error));
            assert_eq!(
                peer.count(),
                3,
                "rejected preparation never sends another completion"
            );
            let decoded = decoded
                .continue_run()
                .err()
                .expect("failure is not continuation");
            let finished = decoded.finish_run(BUDGET).await.unwrap();
            assert_eq!(finished.completion.err(), Some(error));
            assert_eq!(finished.totals.total_steps, 1);
            assert_eq!(finished.totals.total_credits, 13);
            let hits = peer.hits.lock().unwrap();
            let finish: Value = serde_json::from_slice(&hits[3].body).unwrap();
            assert_eq!(finish["status"], "failed");
            assert_eq!(finish["runId"], "original-run");
            assert!(!String::from_utf8_lossy(&hits[3].body).contains("synthetic-secret"));
        })
        .await;
        assert_eq!(row(&db).await, original);
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_catalog_step_owner_redirect_keeps_original_unknown_run_without_replay() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let mut redirect = fixture::Spec::json(307, json!({}));
    redirect.headers.insert(
        "location",
        "https://must-not-follow.invalid/".parse().unwrap(),
    );
    peer.push(redirect);
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let decoded = run
            .execute_step(intent(), 4096, BUDGET, std::future::pending())
            .await;
        assert_eq!(
            decoded.completion().err(),
            Some(BusinessError::RedirectRejected)
        );
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
        let decoded = decoded.continue_run().err().unwrap();
        assert_eq!(
            decoded.finish_run(BUDGET).await.err(),
            Some(BusinessError::AccountingUnknown)
        );
        assert_eq!(
            peer.count(),
            3,
            "no redirect, FINISH, retry, fallback or new START"
        );
        other.close().await;
    })
    .await;
    assert_eq!(row(&db).await, original);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
