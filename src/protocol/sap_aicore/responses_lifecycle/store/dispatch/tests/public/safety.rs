//! Public failure recovery, admission and concurrent mutation contracts.
use super::*;
use crate::pipeline::{stored_responses, PipelineContext};

fn terminal() -> Value {
    json!({"id":"resp_created","object":"response","status":"cancelled","store":true,
        "output":[],"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}})
}

async fn policy_call(
    fixture: &Fixture,
    id: &str,
    policy: Value,
) -> Result<Value, crate::error::GatewayError> {
    let req = crate::protocol::responses::normalize_responses(json!({"input":[]})).unwrap();
    let mut ctx = PipelineContext::new(req, Some(fixture.token.clone()));
    ctx.route_policy_config = Some(serde_json::from_value(policy).unwrap());
    Box::pin(stored_responses::run(
        ctx,
        AuthRequest {
            authorization: Some(format!("Bearer {}", fixture.token)),
            api_key: None,
            path: format!("/v1/responses/{id}"),
            method: "GET".into(),
        },
        stored_responses::Request::Existing {
            id: id.into(),
            operation: Operation::Retrieve,
        },
        fixture.state.clone(),
    ))
    .await
}

#[tokio::test]
async fn sap_aicore_stored_http_scoped_key_and_concurrent_cancel_settle_once() {
    Box::pin(async {
        let wire = Arc::new(Wire {
            create_response: Some(json!({"id":"resp_created","object":"response","status":"queued",
                "store":true,"output":[]})),
            terminal_response: Some(terminal()),
            ..Wire::default()
        });
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        sqlx::query("UPDATE local_access_keys SET payload=json_set(payload,'$.metadata.scopes',json('[\"responses\"]'))")
            .execute(pool).await.unwrap();
        test_support::origin(origin, async {
            let (status, body) = read(fixture.open("responses", create()).await).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let path = format!("/v1/responses/{}", body["id"].as_str().unwrap());
            let cancel = format!("{path}/cancel");
            let (a, b) = tokio::join!(at(&fixture, "POST", &cancel), at(&fixture, "POST", &cancel));
            assert_eq!(usize::from(a.0.is_success()) + usize::from(b.0.is_success()), 1, "{a:?} {b:?}");
            let (a, b) = tokio::join!(at(&fixture, "GET", &path), at(&fixture, "GET", &path));
            assert_eq!((a.0, b.0), (StatusCode::OK, StatusCode::OK));
            assert_eq!(fixture.remaining().await, 1);
            assert_eq!(wire.requests.lock().unwrap().iter().filter(|(_, p)| p.ends_with("/cancel")).count(), 1);
            assert_eq!(at(&fixture, "DELETE", &path).await.0, StatusCode::OK);
            assert_eq!(fixture.remaining().await, 1);
        }).await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_stored_http_missing_ids_rate_limited_and_policy_pin_denied() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let policy = json!({"rateLimitEnforcementVersion":"v1","rateLimitWindowSeconds":60,
            "rateLimitMaxRequests":1});
        let missing = format!("resp_gw_{}", "f".repeat(32));
        assert_eq!(
            policy_call(&fixture, &missing, policy.clone())
                .await
                .unwrap_err()
                .http_status,
            Some(404)
        );
        assert_eq!(
            policy_call(&fixture, &missing, policy)
                .await
                .unwrap_err()
                .http_status,
            Some(429)
        );
        assert!(wire.requests.lock().unwrap().is_empty());
        test_support::origin(origin, async {
            let (status, body) = read(fixture.open("responses", create()).await).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let id = body["id"].as_str().unwrap();
            let error = policy_call(
                &fixture,
                id,
                json!({"allowedProviderAccountIds":["not-original"]}),
            )
            .await
            .unwrap_err();
            assert_eq!(
                error.code.as_deref(),
                Some("sap_aicore_response_route_denied")
            );
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
            assert_eq!(fixture.remaining().await, 1);
        })
        .await;
        let audits = fixture.audits(4).await;
        assert_eq!(
            audits
                .iter()
                .filter(|a| a.provider_account_id.is_none())
                .count(),
            3
        );
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_http_settlement_error_exposes_only_local_recovery_id() {
    settlement_recovery(false).await;
}

#[tokio::test]
async fn sap_aicore_stored_http_message_recovery_without_usage_never_replays() {
    settlement_recovery(true).await;
}

async fn settlement_recovery(without_usage: bool) {
    Box::pin(async {
        let mut output = terminal();
        output["status"] = json!("completed");
        if without_usage {
            output.as_object_mut().unwrap().remove("usage");
        }
        let wire = Arc::new(Wire {
            create_response: Some(output.clone()),
            terminal_response: Some(output),
            ..Wire::default()
        });
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        sqlx::raw_sql("CREATE TRIGGER deny_settlement BEFORE UPDATE OF open ON gateway_sap_response_balance_holds
            BEGIN SELECT RAISE(ABORT,'synthetic settlement failure'); END").execute(pool).await.unwrap();
        test_support::origin(origin, async {
            let (status, body) = read(fixture.open("responses", create()).await).await;
            assert!(!status.is_success());
            assert_eq!(body["error"]["code"], "sap_aicore_response_settlement_pending");
            let id: String = sqlx::query_scalar("SELECT id FROM gateway_sap_response_receipts")
                .fetch_one(pool).await.unwrap();
            assert!(body.to_string().contains(&id), "{body}");
            assert!(!body.to_string().contains("resp_created"));
            let pending = crate::protocol::sap_aicore::responses_lifecycle::store::reconciliation::list(&fixture.state, None).await.unwrap();
            let pending = serde_json::to_value(pending).unwrap();
            assert_eq!(pending[0]["id"], id);
            assert_eq!(pending[0]["balanceOpen"], true);
            assert_eq!(pending[0]["upstreamKnown"], true);
            let oauth = wire.oauth_calls.load(std::sync::atomic::Ordering::Relaxed);
            assert_eq!(oauth, 1);
            sqlx::raw_sql("DROP TRIGGER deny_settlement").execute(pool).await.unwrap();
            crate::protocol::sap_aicore::responses_lifecycle::store::recovery::spawn(&fixture.state);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let settled: i64 = sqlx::query_scalar("SELECT settled FROM gateway_sap_response_receipts WHERE id=$1")
                        .bind(&id).fetch_one(pool).await.unwrap();
                    if settled == 1 { break; }
                    tokio::task::yield_now().await;
                }
            }).await.unwrap();
            assert_eq!(wire.requests.lock().unwrap().len(), 1, "recovery must not send GET");
            assert_eq!(wire.oauth_calls.load(std::sync::atomic::Ordering::Relaxed), oauth,
                "recovery must not request OAuth");
            let receipt: (String,i64,Option<i64>,Option<i64>,i64) = sqlx::query_as(
                "SELECT r.phase,r.settled,r.input_tokens,r.output_tokens,h.open
                 FROM gateway_sap_response_receipts r JOIN gateway_sap_response_balance_holds h
                 ON h.response_id=r.id WHERE r.id=$1").bind(&id).fetch_one(pool).await.unwrap();
            assert_eq!((receipt.0.as_str(), receipt.1, receipt.4), ("completed", 1, 0));
            if without_usage {
                assert_eq!((receipt.2, receipt.3), (None, None));
            } else {
                assert!(receipt.2.is_some() && receipt.3.is_some());
            }
            assert_eq!(fixture.remaining().await, 1);
            // Keep the original public retry coverage after autonomous settlement.
            for _ in 0..2 {
                assert_eq!(at(&fixture, "GET", &format!("/v1/responses/{id}")).await.0, StatusCode::OK);
                assert_eq!(fixture.remaining().await, 1);
            }
            fixture.state.shutdown.request("recovery fixture done");
            fixture.state.lifecycle.background_tasks.join().await.unwrap();
            assert_eq!(fixture.remaining().await, 1);
            let audit = fixture.audits(3).await;
            assert!(audit.iter().all(|a| a.route_trace.as_ref().unwrap()["storedResponseId"] == id));
        }).await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_stored_http_projection_error_retains_recoverable_id() {
    Box::pin(async {
        let mut output = terminal();
        output["output"] = json!([{"type":"unknown_private_item","secret":"must-not-leak"}]);
        let wire = Arc::new(Wire {
            create_response: Some(output),
            terminal_response: Some(terminal()),
            ..Wire::default()
        });
        let (origin, fixture, server) = prepare(wire.clone()).await;
        test_support::origin(origin, async {
            let (status, body) = read(fixture.open("responses", create()).await).await;
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
            let id: String = sqlx::query_scalar("SELECT id FROM gateway_sap_response_receipts")
                .fetch_one(pool)
                .await
                .unwrap();
            assert!(body.to_string().contains(&id));
            assert!(!body.to_string().contains("must-not-leak"));
            assert_eq!(
                at(&fixture, "GET", &format!("/v1/responses/{id}")).await.0,
                StatusCode::OK
            );
            assert_eq!(fixture.remaining().await, 1);
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
