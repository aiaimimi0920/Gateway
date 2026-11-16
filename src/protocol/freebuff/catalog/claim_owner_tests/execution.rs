//! Real SQLite ownership and loopback START/completion, not a full agent or public inference test.
use super::*;
use futures::StreamExt;

#[tokio::test]
async fn freebuff_catalog_run_pins_claim_rotated_handle_real_run_and_signed_final_bytes() {
    let (root, db) = setup().await;
    let (mut session, device) = account();
    let old = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &old).await;
    let claim = prepared.claim().clone();
    let persisted = row(&db).await;
    drop(prepared);
    drop(old);
    let mut next = tests::body();
    next["rows"][0]["handle"] = json!("fbm1.rotated");
    next["fetchId"] = json!("rotated-fetch");
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&next).unwrap()).unwrap());
    let requests = session.business_requests(&device, "open").unwrap();
    let mut reply = fixture::Spec::json(200, json!({"transportOnly":true}));
    reply.body_delay = Duration::from_secs(10);
    let peer = fixture::Peer::start(vec![
        fixture::Spec::json(200, active(&claim, "open")),
        fixture::Spec::json(200, json!({"runId":"actual-server-run"})),
        reply,
    ])
    .await;
    let other = LocalRuntime::open(&root).await.unwrap();
    with_test_origin(peer.origin.clone(), async {
        let mut admitted = recovery(&db, &requests.recovery_requests())
            .await
            .heartbeat(&requests.recovery_requests(), BUDGET)
            .await
            .unwrap();
        let run = admitted.start_catalog_run(&requests, BUDGET).await.unwrap();
        assert!(!format!("{run:?}").contains("actual-server-run"));
        let mut input = json!({"model":"fbm1.rotated", "stream":true,
            "messages":[{"role":"user","content":"fixture, not an agent harness"}],
            "codebuff_metadata":{"custom":"keep","run_id":"foreign","client_id":"foreign",
                "cost_mode":"paid", "surface":"desktop", "freebuff_instance_id":"foreign",
                "freebuff_multi_session":"0", "freebuff_unknown":"foreign"}});
        let frozen = run.prepare_completion(input.clone(), 32768).unwrap();
        input["messages"][0]["content"] = json!("changed after freeze");
        let response = frozen.send(BUDGET).await.unwrap();
        assert_eq!(
            response.status(),
            200,
            "transport status is not inference success"
        );
        assert!(response.headers().get("content-type").is_some());
        let body = response.into_body();
        assert!(!format!("{body:?}").contains("actual-server-run"));
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
        {
            let hits = peer.hits.lock().unwrap();
            assert_eq!(hits.len(), 3);
            assert_eq!(hits[1].method, "POST");
            assert_eq!(hits[1].uri.path(), "/api/v1/agent-runs");
            assert_eq!(
                serde_json::from_slice::<Value>(&hits[1].body).unwrap(),
                json!({"action":"START", "agentId":"base3-free-catalog", "ancestorRunIds":[]})
            );
            assert!(
                hits[1]
                    .headers
                    .keys()
                    .all(|name| !name.as_str().starts_with("x-freebuff-")),
                "START must not inherit device/session/acting-user headers"
            );
            assert_eq!(hits[1].headers["content-type"], "text/plain;charset=UTF-8");
            assert_eq!(hits[1].headers["authorization"], "Bearer synthetic-account");
            let sent: Value = serde_json::from_slice(&hits[2].body).unwrap();
            assert_eq!(sent["model"], "fbm1.rotated");
            assert_eq!(
                sent["messages"][0]["content"],
                "fixture, not an agent harness"
            );
            let metadata = &sent["codebuff_metadata"];
            assert_eq!(metadata["run_id"], "actual-server-run");
            assert_eq!(metadata["freebuff_instance_id"], claim.instance_id());
            assert_eq!(metadata["freebuff_multi_session"], "1");
            assert_eq!(metadata["surface"], "cli");
            assert_eq!(metadata["cost_mode"], "free");
            assert_eq!(metadata["custom"], "keep");
            assert!(metadata.get("freebuff_unknown").is_none());
            assert_ne!(metadata["client_id"], "foreign");
            use base64::Engine;
            let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(device.key.public_key())
                .unwrap();
            business_tests::verify(&hits[0], &public, "rotated-fetch");
            business_tests::verify(&hits[2], &public, "rotated-fetch");
        }
        drop(body);
        drop(admitted);
        assert_eq!(
            row(&db).await,
            persisted,
            "completion/drop never clears a purchase obligation"
        );
        drop(recovery(&other, &requests.recovery_requests()).await);
        assert_eq!(
            peer.count(),
            3,
            "no implicit FINISH, admission replay or alternate request"
        );
    })
    .await;
    other.close().await;
    peer.close().await;
    cleanup(&root, db).await;
}

fn observation(requests: &BusinessRequests<'_>, remaining: f64) -> OwnedActiveSession {
    let claim = requests
        .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
        .unwrap()
        .claim()
        .clone();
    let mut body = active(&claim, requests.selected.key);
    body["remainingMs"] = json!(remaining);
    OwnedActiveSession::parse(
        BusinessReply { status: 200, body },
        &claim,
        requests.selected.key,
        OffsetDateTime::now_utc(),
    )
    .unwrap()
}

#[tokio::test]
async fn freebuff_catalog_run_rejects_foreign_authority_model_and_stale_observation_before_start() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let live = observation(&requests, 3600000.0);
    let peer = fixture::Peer::start(vec![]).await;
    with_test_origin(peer.origin.clone(), async {
        for (token, client, origin) in [
            ("other", ClientSurface::Cli, BusinessOrigin::Codebuff),
            (
                "synthetic-account",
                ClientSurface::Desktop,
                BusinessOrigin::Codebuff,
            ),
            (
                "synthetic-account",
                ClientSurface::Cli,
                BusinessOrigin::CodebuffWww,
            ),
        ] {
            let (foreign, key) = synthetic(token, client, origin);
            let wrong = foreign.business_requests(&key, "open").unwrap();
            assert_eq!(
                CatalogRun::start(&wrong, &live, BUDGET).await.err(),
                Some(CatalogError::ScopeMismatch.into())
            );
        }
        let (mut other, key) = account();
        let mut changed = tests::body();
        changed["rows"][0]["key"] = json!("different-model");
        other.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&changed).unwrap()).unwrap());
        let wrong = other.business_requests(&key, "different-model").unwrap();
        assert_eq!(
            CatalogRun::start(&wrong, &live, BUDGET).await.err(),
            Some(CatalogError::ScopeMismatch.into())
        );
        let stale = observation(&requests, 1.0);
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            CatalogRun::start(&requests, &stale, BUDGET).await.err(),
            Some(BusinessError::SessionInactive)
        );
        assert_eq!(
            CatalogRun::start(&requests, &live, Duration::ZERO)
                .await
                .err(),
            Some(BusinessError::InvalidInput)
        );
        assert_eq!(peer.count(), 0);
    })
    .await;
    peer.close().await;
}

#[tokio::test]
async fn freebuff_catalog_run_unknown_start_is_not_retried_or_guessed_from_a_200() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let live = observation(&requests, 3600000.0);
    for (status, body, expected) in [
        (
            503,
            json!({"secret":"must not escape"}),
            BusinessError::RunUnconfirmed,
        ),
        (200, json!({"runId":""}), BusinessError::RunUnconfirmed),
        (200, json!({"runId":37}), BusinessError::RunUnconfirmed),
        (
            200,
            json!({"runId":"x\r\nsecret"}),
            BusinessError::RunUnconfirmed,
        ),
        (
            200,
            json!({"runId":"x".repeat(65536)}),
            BusinessError::ResponseTooLarge,
        ),
        (
            307,
            json!({"runId":"must-not-follow"}),
            BusinessError::RedirectRejected,
        ),
    ] {
        let mut spec = fixture::Spec::json(status, body);
        spec.headers
            .insert("location", "/api/v1/agent-runs".parse().unwrap());
        let peer = fixture::Peer::start(vec![spec]).await;
        with_test_origin(peer.origin.clone(), async {
            let error = CatalogRun::start(&requests, &live, BUDGET)
                .await
                .err()
                .unwrap();
            assert_eq!(error, expected);
            assert!(!error.to_string().contains("secret"));
            assert_eq!(peer.count(), 1);
        })
        .await;
        peer.close().await;
    }
}

#[tokio::test]
async fn freebuff_catalog_run_rechecks_expiry_after_start_and_before_frozen_send() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![fixture::Spec::json(200, json!({"runId":"real"}))]).await;
    with_test_origin(peer.origin.clone(), async {
        let live = observation(&requests, 1000.0);
        let run = CatalogRun::start(&requests, &live, BUDGET).await.unwrap();
        let prepared = run
            .prepare_completion(json!({"model":"fbm1.synthetic-open"}), 32768)
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(
            prepared.send(BUDGET).await.err(),
            Some(BusinessError::SessionInactive)
        );
        assert_eq!(
            peer.count(),
            1,
            "expiry cannot send an already-frozen request"
        );
    })
    .await;
    peer.close().await;
    let mut delayed = fixture::Spec::json(200, json!({"runId":"late"}));
    delayed.header_delay = Duration::from_millis(100);
    let peer = fixture::Peer::start(vec![delayed]).await;
    with_test_origin(peer.origin.clone(), async {
        let live = observation(&requests, 10.0);
        assert_eq!(
            CatalogRun::start(&requests, &live, BUDGET).await.err(),
            Some(BusinessError::SessionInactive)
        );
        assert_eq!(peer.count(), 1);
    })
    .await;
    peer.close().await;
}

#[tokio::test]
async fn freebuff_catalog_run_completion_body_keeps_the_original_transport_timeout() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let live = observation(&requests, 3600000.0);
    let mut delayed = fixture::Spec::json(200, json!({"choices":[]}));
    delayed.body_delay = Duration::from_millis(300);
    let peer = fixture::Peer::start(vec![
        fixture::Spec::json(200, json!({"runId":"real"})),
        delayed,
    ])
    .await;
    with_test_origin(peer.origin.clone(), async {
        let run = CatalogRun::start(&requests, &live, BUDGET).await.unwrap();
        let response = run
            .prepare_completion(json!({"model":"fbm1.synthetic-open"}), 32768)
            .unwrap()
            .send(Duration::from_millis(50))
            .await
            .unwrap();
        let mut body = response.into_body();
        let error = body.next().await.unwrap().unwrap_err();
        assert!(error.is_timeout());
        drop(body);
        assert_eq!(
            peer.count(),
            2,
            "a body failure cannot start a replacement run"
        );
    })
    .await;
    peer.close().await;
}

#[tokio::test]
async fn freebuff_catalog_run_preparation_rejects_wrong_handle_bad_metadata_and_encoded_overflow() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let live = observation(&requests, 3600000.0);
    for body in [
        json!({"model":"open"}),
        json!({"model":"fbm1.other"}),
        json!(null),
        json!({"model":"fbm1.synthetic-open","codebuff_metadata":"invalid"}),
    ] {
        let peer =
            fixture::Peer::start(vec![fixture::Spec::json(200, json!({"runId":"real"}))]).await;
        with_test_origin(peer.origin.clone(), async {
            let run = CatalogRun::start(&requests, &live, BUDGET).await.unwrap();
            assert_eq!(
                run.prepare_completion(body, 32768).err(),
                Some(BusinessError::InvalidInput)
            );
            assert_eq!(peer.count(), 1);
        })
        .await;
        peer.close().await;
    }
    let peer = fixture::Peer::start(vec![fixture::Spec::json(200, json!({"runId":"real"}))]).await;
    with_test_origin(peer.origin.clone(), async {
        let run = CatalogRun::start(&requests, &live, BUDGET).await.unwrap();
        assert_eq!(
            run.prepare_completion(json!({"model":"fbm1.synthetic-open"}), 2)
                .err(),
            Some(BusinessError::RequestTooLarge)
        );
        assert_eq!(peer.count(), 1);
    })
    .await;
    peer.close().await;
}

#[tokio::test]
async fn freebuff_catalog_run_cancelled_start_retains_original_purchase_without_cleanup_or_replay()
{
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    let claim = prepared.claim().clone();
    let saved = row(&db).await;
    drop(prepared);
    let mut delayed = fixture::Spec::json(200, json!({"runId":"unreceived"}));
    delayed.header_delay = Duration::from_secs(1);
    let peer = fixture::Peer::start(vec![
        fixture::Spec::json(200, active(&claim, "open")),
        delayed,
    ])
    .await;
    with_test_origin(peer.origin.clone(), async {
        let mut admitted = recovery(&db, &requests.recovery_requests())
            .await
            .heartbeat(&requests.recovery_requests(), BUDGET)
            .await
            .unwrap();
        {
            let start = admitted.start_catalog_run(&requests, BUDGET);
            tokio::pin!(start);
            tokio::select! {
                result = &mut start => panic!("START must still await headers: {result:?}"),
                _ = tokio::time::timeout(BUDGET, async {
                    while peer.count() < 2 { tokio::task::yield_now().await; }
                }) => {}
            }
        }
        drop(admitted);
        assert_eq!(row(&db).await, saved);
        assert_eq!(
            recovery(&db, &requests.recovery_requests())
                .await
                .claim()
                .attempt_id(),
            claim.attempt_id()
        );
        assert_eq!(peer.count(), 2);
    })
    .await;
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_run_start_budget_covers_headers_and_body_without_automatic_retry() {
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let live = observation(&requests, 3600000.0);
    for headers in [true, false] {
        let mut delayed = fixture::Spec::json(200, json!({"runId":"late"}));
        if headers {
            delayed.header_delay = Duration::from_millis(200);
        } else {
            delayed.body_delay = Duration::from_millis(200);
        }
        let peer = fixture::Peer::start(vec![delayed]).await;
        with_test_origin(peer.origin.clone(), async {
            assert_eq!(
                CatalogRun::start(&requests, &live, Duration::from_millis(40))
                    .await
                    .err(),
                Some(BusinessError::Timeout)
            );
            assert_eq!(peer.count(), 1);
        })
        .await;
        peer.close().await;
    }
}
