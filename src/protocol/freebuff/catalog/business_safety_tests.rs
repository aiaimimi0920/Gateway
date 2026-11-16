use super::{business_tests::ready, fixture::*, tests::body, wire_tests::*, *};
use serde_json::json;
const BUDGET: Duration = Duration::from_secs(3);

#[tokio::test]
async fn rejected_http_results_and_unknown_posts_never_retry_or_fall_back() {
    for status in [401, 403, 404, 405, 408, 409, 429, 500, 503] {
        let response = json!({"error":"sensitive upstream echo","status":"not-an-admission"});
        let peer = Peer::start(vec![
            Spec::catalog(),
            Spec::json(200, json!({"keyId":"device"})),
            Spec::json(status, response.clone()),
        ])
        .await;
        with_test_origin(peer.origin.clone(), async {
            let (catalog, device, _) = ready(&peer, ClientSurface::Cli).await;
            let scope = catalog.business_requests(&device, "open").unwrap();
            let result = scope
                .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
                .unwrap()
                .send(BUDGET)
                .await
                .unwrap();
            assert_eq!(result.status, status);
            assert_eq!(
                result.body, response,
                "raw protocol reply is not interpreted as admission success"
            );
            assert!(!format!("{result:?}").contains("sensitive"));
            assert_eq!(peer.count(), 3, "no fallback, stale refresh or POST retry");
        })
        .await;
        peer.close().await;
    }
}

#[tokio::test]
async fn redirects_never_forward_secrets_for_signed_session_or_completion() {
    let trap = Peer::start(vec![]).await;
    for status in [301, 302, 303, 307, 308] {
        for completion in [false, true] {
            let mut redirect = Spec::json(status, json!({}));
            redirect
                .headers
                .insert("location", format!("{}/trap", trap.origin).parse().unwrap());
            let peer = Peer::start(vec![
                Spec::catalog(),
                Spec::json(200, json!({"keyId":"device"})),
                redirect,
            ])
            .await;
            with_test_origin(peer.origin.clone(), async {
                let (catalog, device, _) = ready(&peer, ClientSurface::Cli).await;
                let scope = catalog.business_requests(&device, "open").unwrap();
                let error = if completion {
                    scope
                        .prepare_completion(&json!({"model":"fbm1.synthetic-open"}), 32768)
                        .unwrap()
                        .send(BUDGET)
                        .await
                        .err()
                } else {
                    scope
                        .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
                        .unwrap()
                        .send(BUDGET)
                        .await
                        .err()
                };
                assert_eq!(error, Some(BusinessError::RedirectRejected));
                assert_eq!(peer.count(), 3);
                assert_eq!(trap.count(), 0);
            })
            .await;
            peer.close().await;
        }
    }
    trap.close().await;
}

#[tokio::test]
async fn missing_catalog_foreign_device_and_foreign_claim_fail_before_io() {
    let peer = Peer::start(vec![
        Spec::catalog(),
        Spec::json(200, json!({"keyId":"device"})),
    ])
    .await;
    with_test_origin(peer.origin.clone(), async {
        let (catalog, device, _) = ready(&peer, ClientSurface::Cli).await;
        let scope = catalog.business_requests(&device, "open").unwrap();
        for key in ["locked", "future", "missing", "fbm1.synthetic-open"] {
            assert!(matches!(
                catalog.business_requests(&device, key),
                Err(BusinessError::Catalog(CatalogError::ModelUnavailable))
            ));
        }
        let claim = scope
            .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
            .unwrap()
            .claim()
            .clone();
        for (origin, token, client) in [
            (
                BusinessOrigin::Codebuff,
                "synthetic-token",
                ClientSurface::Cli,
            ),
            (
                BusinessOrigin::CodebuffWww,
                "changed-token",
                ClientSurface::Cli,
            ),
            (
                BusinessOrigin::CodebuffWww,
                "synthetic-token",
                ClientSurface::Desktop,
            ),
        ] {
            let mut foreign = CatalogSession::new(origin, token, client).unwrap();
            assert!(matches!(
                foreign.business_requests(&device, "open"),
                Err(BusinessError::Catalog(CatalogError::ScopeMismatch))
            ));
            let foreign_device =
                DeviceRegistration::new(&foreign.binding, key().0, "other-device".into());
            assert!(matches!(
                foreign.business_requests(&foreign_device, "open"),
                Err(BusinessError::Catalog(CatalogError::CatalogRequired))
            ));
            foreign.catalog =
                Some(ModelCatalog::parse(&serde_json::to_vec(&body()).unwrap()).unwrap());
            let scope = foreign.business_requests(&foreign_device, "open").unwrap();
            assert!(matches!(
                scope.heartbeat(&claim, BUDGET).await,
                Err(BusinessError::Catalog(CatalogError::ScopeMismatch))
            ));
            assert!(matches!(
                scope.end(&claim, BUDGET).await,
                Err(BusinessError::Catalog(CatalogError::ScopeMismatch))
            ));
            if client == ClientSurface::Desktop {
                assert!(matches!(
                    scope.prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks),
                    Err(BusinessError::Catalog(CatalogError::Unsupported))
                ));
            }
        }
        assert_eq!(peer.count(), 2);
    })
    .await;
    peer.close().await;
}

#[tokio::test]
async fn session_deadline_covers_headers_and_decoded_body_and_keeps_claim_for_reconciliation() {
    for (header_ms, body_ms) in [(300, 0), (0, 300), (120, 180)] {
        let mut delayed = Spec::json(200, json!({"status":"active"}));
        delayed.header_delay = Duration::from_millis(header_ms);
        delayed.body_delay = Duration::from_millis(body_ms);
        let peer = Peer::start(vec![
            Spec::catalog(),
            Spec::json(200, json!({"keyId":"device"})),
            delayed,
            Spec::json(200, json!({"status":"none"})),
        ])
        .await;
        with_test_origin(peer.origin.clone(), async {
            let (catalog, device, _) = ready(&peer, ClientSurface::Cli).await;
            let scope = catalog.business_requests(&device, "open").unwrap();
            let request = scope
                .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
                .unwrap();
            let claim = request.claim().clone();
            assert_eq!(
                request.send(Duration::from_millis(200)).await.err(),
                Some(BusinessError::Timeout)
            );
            scope.heartbeat(&claim, BUDGET).await.unwrap();
            let hits = peer.hits.lock().unwrap();
            assert_eq!(hits.len(), 4);
            assert_eq!(
                hits[2].headers["x-freebuff-instance-id"],
                hits[3].headers["x-freebuff-instance-id"]
            );
            assert_eq!(hits[3].method, "GET");
            assert_eq!(hits[3].headers["x-freebuff-heartbeat"], "1");
        })
        .await;
        peer.close().await;
    }
}

#[tokio::test]
async fn cancelling_admission_drops_body_without_a_detached_retry_or_implicit_end() {
    let mut delayed = Spec::json(200, json!({"status":"active"}));
    delayed.body_delay = Duration::from_secs(60);
    let peer = Peer::start(vec![
        Spec::catalog(),
        Spec::json(200, json!({"keyId":"device"})),
        delayed,
    ])
    .await;
    with_test_origin(peer.origin.clone(),async {
        let (catalog,device,_) = ready(&peer,ClientSurface::Cli).await;
        let scope = catalog.business_requests(&device,"open").unwrap();
        let request = scope.prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks).unwrap();
        let claim = request.claim().clone();
        let started = Arc::new(tokio::sync::Notify::new());
        let mut pending = Box::pin(TEST_BODY_STARTED.scope(started.clone(),request.send(BUDGET)));
        tokio::time::timeout(BUDGET,async { tokio::select! { _ = started.notified() => {}, result = &mut pending => panic!("unexpected: {result:?}") } }).await.unwrap();
        drop(pending);
        peer.wait_drops(3).await;
        assert!(claim.instance_id().starts_with("cli:"));
        assert_eq!(peer.count(),3,"drop does not buy, retry or invent a confirmed release/refund");
    }).await;
    peer.close().await;
}

#[tokio::test]
async fn session_decoded_body_is_bounded_and_malformed_answers_are_not_success() {
    for body in [vec![b'x'; 1_048_577], b"not json sensitive-token".to_vec()] {
        let large = body.len() > 1_048_576;
        let mut reply = Spec::json(200, json!({}));
        reply.body = body;
        let peer = Peer::start(vec![
            Spec::catalog(),
            Spec::json(200, json!({"keyId":"device"})),
            reply,
        ])
        .await;
        with_test_origin(peer.origin.clone(), async {
            let (catalog, device, _) = ready(&peer, ClientSurface::Cli).await;
            let scope = catalog.business_requests(&device, "open").unwrap();
            let error = scope
                .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
                .unwrap()
                .send(BUDGET)
                .await
                .unwrap_err();
            assert_eq!(
                error,
                if large {
                    BusinessError::ResponseTooLarge
                } else {
                    BusinessError::InvalidResponse
                }
            );
            assert!(!format!("{error:?} {error}").contains("sensitive-token"));
            assert_eq!(peer.count(), 3);
        })
        .await;
        peer.close().await;
    }
}

#[tokio::test]
async fn completion_body_timeout_and_drop_are_owned_by_the_returned_response() {
    use futures::StreamExt;
    for timeout in [false, true] {
        let mut delayed = Spec::json(200, json!({"choices":[]}));
        delayed.body_delay = Duration::from_secs(60);
        let peer = Peer::start(vec![
            Spec::catalog(),
            Spec::json(200, json!({"keyId":"device"})),
            delayed,
        ])
        .await;
        with_test_origin(peer.origin.clone(), async {
            let (catalog, device, _) = ready(&peer, ClientSurface::Cli).await;
            let scope = catalog.business_requests(&device, "open").unwrap();
            let response = scope
                .prepare_completion(&json!({"model":"fbm1.synthetic-open","stream":true}), 32768)
                .unwrap()
                .send(Duration::from_millis(200))
                .await
                .unwrap();
            if timeout {
                let chunk = tokio::time::timeout(BUDGET, response.bytes_stream().next())
                    .await
                    .unwrap()
                    .unwrap();
                assert!(
                    chunk.unwrap_err().is_timeout(),
                    "request timeout continues through body consumption"
                );
            } else {
                drop(response);
            }
            peer.wait_drops(3).await;
            assert_eq!(peer.count(), 3);
        })
        .await;
        peer.close().await;
    }
}
