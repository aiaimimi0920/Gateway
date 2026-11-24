//! Admission failures retain their audit owner and do not consume unrelated buckets.
use super::*;
use crate::pipeline::{stage_auth, stage_rate_limit, stored_responses, PipelineContext};

fn auth(fixture: &Fixture) -> AuthRequest {
    AuthRequest {
        authorization: Some(format!("Bearer {}", fixture.token)),
        api_key: None,
        path: "/v1/responses".into(),
        method: "POST".into(),
    }
}

fn context(fixture: &Fixture, policy: Value) -> PipelineContext {
    let request = crate::protocol::responses::normalize_responses(create()).unwrap();
    let mut ctx = PipelineContext::new(request, Some(fixture.token.clone()));
    ctx.route_policy_config = Some(serde_json::from_value(policy).unwrap());
    ctx
}

#[tokio::test]
async fn sap_aicore_stored_http_rejected_tracker_finishes_create_audit() {
    Box::pin(async {
        for closed in [false, true] {
            let wire = Arc::new(Wire::default());
            let (_, fixture, server) = prepare(wire.clone()).await;
            let tracker = &fixture.state.lifecycle.stored_response_tasks;
            let mut slots = Vec::new();
            if closed {
                tracker.close();
            } else {
                while let Ok(slot) = tracker.try_admit() {
                    slots.push(slot);
                }
                assert_eq!(slots.len(), 32);
            }
            let (status, body) = read(fixture.open("responses", create()).await).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            let expected = if closed {
                "sap_aicore_response_draining"
            } else {
                "sap_aicore_response_busy"
            };
            assert_eq!(body["error"]["code"], expected, "{body}");
            assert_eq!(fixture.remaining().await, 2);
            assert!(wire.requests.lock().unwrap().is_empty());
            let audits = fixture.audits(1).await;
            assert_eq!(
                audits[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
                0
            );
            assert!(audits[0].provider_account_id.is_none());
            assert!(audits[0].route_trace.as_ref().unwrap()["realCredentialRef"].is_null());
            drop(slots);
            fixture.finish().await;
            server.abort();
            let _ = server.await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_http_continuation_checks_circuit_but_cleanup_remains_available() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (origin, fixture, server) = prepare(wire.clone()).await;
        test_support::origin(origin, async {
            let (status, response) = read(fixture.open("responses", create()).await).await;
            assert_eq!(status, StatusCode::OK, "{response}");
            let id = response["id"].as_str().unwrap();
            let candidate = fixture
                .state
                .route_config
                .snapshot()
                .resolve_candidates_with_constraint(Some(MODEL), None)
                .candidates
                .into_iter()
                .find(|c| c.payload.credential_id.as_deref() == Some("exact"))
                .unwrap();
            let attempt =
                crate::routing::admission::attempt(&fixture.state, &candidate, "deployment1");
            attempt.begin().unwrap();
            attempt.finish(crate::routing::model_circuit::Outcome::ChannelUnavailable(
                60_000,
            ));
            let mut next = create();
            next["previous_response_id"] = id.into();
            let (status, rejected) = read(fixture.open("responses", next).await).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{rejected}");
            assert_eq!(rejected["error"]["code"], "model_circuit_open");
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
            let path = format!("/v1/responses/{id}");
            assert_eq!(at(&fixture, "GET", &path).await.0, StatusCode::OK);
            assert_eq!(at(&fixture, "DELETE", &path).await.0, StatusCode::OK);
            assert_eq!(fixture.remaining().await, 1);
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_http_model_rejection_preserves_public_rate_buckets() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (_, fixture, server) = prepare(wire.clone()).await;
        let model = json!({"rateLimitEnforcementVersion":"v1",
            "rateLimitWindowSeconds":null,"rateLimitMaxRequests":null,
            "modelRateLimits":{MODEL:{"windowSeconds":60,"maxRequests":1}}});
        let public = json!({"rateLimitEnforcementVersion":"v1",
            "rateLimitWindowSeconds":60,"rateLimitMaxRequests":1,
            "apiKeyRateLimit":{"windowSeconds":60,"maxRequests":1},
            "endpointRateLimits":{"responses":{"windowSeconds":60,"maxRequests":1}}});
        let mut warm = context(&fixture, model.clone());
        stage_auth::run(&mut warm, &fixture.state).await.unwrap();
        stage_rate_limit::run(&mut warm, &fixture.state)
            .await
            .unwrap();
        let mut combined = public.clone();
        combined["modelRateLimits"] = model["modelRateLimits"].clone();
        let result = Box::pin(stored_responses::create(
            context(&fixture, combined),
            auth(&fixture),
            fixture.state.clone(),
            Default::default(),
        ))
        .await;
        let error = result.err().unwrap();
        assert_eq!(error.http_status, Some(429));
        assert!(error.message.contains("model scope"), "{error:?}");
        let mut probe = context(&fixture, public);
        stage_auth::run(&mut probe, &fixture.state).await.unwrap();
        stage_rate_limit::run(&mut probe, &fixture.state)
            .await
            .unwrap();
        assert!(stage_rate_limit::run(&mut probe, &fixture.state)
            .await
            .is_err());
        assert_eq!(fixture.remaining().await, 2);
        assert!(wire.requests.lock().unwrap().is_empty());
        fixture.audits(1).await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_response_receipts_unsent_refund_error_keeps_local_id() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let session = authenticate_request(&fixture.state, &auth(&fixture), None)
            .await
            .unwrap();
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        sqlx::raw_sql(
            "CREATE TRIGGER deny_refund BEFORE UPDATE OF open ON gateway_sap_response_balance_holds
            BEGIN SELECT RAISE(ABORT,'synthetic refund failure'); END",
        )
        .execute(pool)
        .await
        .unwrap();
        let selected = super::super::super::super::ingress::PinnedTarget {
            public_model: MODEL.into(),
            provider: "sap_aicore-public".into(),
            credential: "exact".into(),
            source_key: session.access_key_id.clone().unwrap(),
            platform_access: None,
            upstream_model: "deployment1".into(),
        };
        let req = crate::protocol::responses::normalize_responses(create()).unwrap();
        test_support::origin(origin, async {
            let error = super::super::super::super::ingress::run_submit(
                &fixture.state,
                &session,
                &selected,
                None,
                &req,
                1,
                SendHooks {
                    begin: &|_| Err(crate::error::GatewayError::service_unavailable("test stop")),
                    ..SendHooks::default()
                },
            )
            .await
            .unwrap_err();
            let id: String = sqlx::query_scalar("SELECT id FROM gateway_sap_response_receipts")
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(
                error.code.as_deref(),
                Some("sap_aicore_response_refund_pending")
            );
            assert!(error.message.contains(&id) && error.message.contains("was not sent"));
            assert!(!error.retryable);
            assert!(wire.requests.lock().unwrap().is_empty());
            assert_eq!(fixture.remaining().await, 1);
            let proof: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM gateway_sap_response_unsent_proofs WHERE response_id=$1",
            )
            .bind(&id)
            .fetch_one(pool)
            .await
            .unwrap();
            assert_eq!(proof, 1);
            sqlx::raw_sql("DROP TRIGGER deny_refund")
                .execute(pool)
                .await
                .unwrap();
            let mut cursor = String::new();
            crate::protocol::sap_aicore::responses_lifecycle::store::recovery::tick(
                &fixture.state,
                &mut cursor,
            )
            .await
            .unwrap();
            crate::protocol::sap_aicore::responses_lifecycle::store::recovery::tick(
                &fixture.state,
                &mut cursor,
            )
            .await
            .unwrap();
            assert_eq!(fixture.remaining().await, 2);
            assert!(wire.requests.lock().unwrap().is_empty());
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
