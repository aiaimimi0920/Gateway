//! HTTP evidence never doubles as refund proof or permission to replay a mutation.
use super::*;
use crate::db;

async fn health(fixture: &Fixture) -> Vec<db::GatewayProviderCredentialModelStateView> {
    fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn sap_aicore_stored_http_failure_health_keeps_reserved_hold() {
    Box::pin(async {
        for status in [400, 401, 403, 429, 503, 200] {
            let wire = Arc::new(Wire::default());
            *wire.rejection.lock().unwrap() = Some((status, b"{private-invalid-json".to_vec()));
            let (origin, fixture, server) = prepare(wire.clone()).await;
            test_support::origin(origin, async {
                let (public, body) = read(fixture.open("responses", create()).await).await;
                assert!(!public.is_success());
                assert_eq!(
                    body["error"]["code"],
                    "sap_aicore_response_delivery_unknown"
                );
                assert!(!body.to_string().contains("private-invalid-json"));
                assert_eq!(
                    fixture.remaining().await,
                    1,
                    "HTTP failure is not unsent proof"
                );
                assert_eq!(wire.requests.lock().unwrap().len(), 1);
                let rows = health(&fixture).await;
                if matches!(status, 429 | 503 | 200) {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].provider_credential_ref.as_deref(), Some("exact"));
                    assert_eq!(rows[0].model, "deployment1");
                    assert_eq!(rows[0].failure_count, 1);
                    assert_eq!(rows[0].last_upstream_status, Some(i32::from(status)));
                    if status == 429 {
                        let until = time::OffsetDateTime::parse(
                            rows[0].cooldown_until.as_deref().unwrap(),
                            &time::format_description::well_known::Rfc3339,
                        )
                        .unwrap();
                        assert!(
                            until - time::OffsetDateTime::now_utc() > time::Duration::seconds(100)
                        );
                    }
                } else {
                    assert!(rows.is_empty());
                }
                let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
                let open: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM gateway_sap_response_balance_holds WHERE open=1",
                )
                .fetch_one(pool)
                .await
                .unwrap();
                assert_eq!(open, 1);
                let audit = fixture.audits(1).await;
                assert_eq!(audit[0].upstream_status, Some(i32::from(status)));
                assert_eq!(
                    audit[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
                    1
                );
                assert_eq!(
                    audit[0].route_trace.as_ref().unwrap()["realCredentialRef"],
                    "exact"
                );
            })
            .await;
            fixture.finish().await;
            server.abort();
            let _ = server.await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_http_mutation_failure_and_get_do_not_unlock_or_clear_health() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (origin, fixture, server) = prepare(wire.clone()).await;
        test_support::origin(origin, async {
            let (_, response) = read(fixture.open("responses", create()).await).await;
            let path = format!("/v1/responses/{}", response["id"].as_str().unwrap());
            *wire.rejection.lock().unwrap() = Some((429, b"private-rejection".to_vec()));
            assert!(!at(&fixture, "DELETE", &path).await.0.is_success());
            let rows = health(&fixture).await;
            assert_eq!(rows[0].failure_count, 1);
            *wire.rejection.lock().unwrap() = None;
            assert_eq!(at(&fixture, "GET", &path).await.0, StatusCode::OK);
            assert_eq!(
                health(&fixture).await[0].failure_count,
                1,
                "GET is not generation recovery"
            );
            assert!(!at(&fixture, "DELETE", &path).await.0.is_success());
            assert_eq!(wire.requests.lock().unwrap().len(), 3);
            assert_eq!(fixture.remaining().await, 1);
            let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
            let pending: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM gateway_sap_response_mutations WHERE confirmed=0",
            )
            .fetch_one(pool)
            .await
            .unwrap();
            assert_eq!(pending, 1);
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_http_task_failure_and_cancellation_are_not_conflated() {
    Box::pin(async {
        for status in ["failed", "cancelled", "queued"] {
            let mut body = json!({"id":"resp_created","object":"response","status":status,"output":[],"store":true});
            if status == "failed" { body["error"] = json!({"code":"server_error","message":"private-error"}); }
            let wire = Arc::new(Wire { create_response: Some(body), ..Wire::default() });
            let (origin, fixture, server) = prepare(wire.clone()).await;
            test_support::origin(origin, async {
                let _ = read(fixture.open("responses", create()).await).await;
                let rows = health(&fixture).await;
                if status == "failed" {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].failure_count, 1);
                    assert_eq!(rows[0].last_upstream_status, Some(200));
                } else { assert!(rows.is_empty(), "{status}: {rows:?}"); }
                assert_eq!(wire.requests.lock().unwrap().len(), 1);
            }).await;
            fixture.finish().await; server.abort(); let _ = server.await;
        }
    }).await;
}

#[tokio::test]
async fn sap_aicore_stored_http_health_storage_failure_does_not_change_receipt_or_balance() {
    Box::pin(async {
        let (origin, fixture, server) = prepare(Arc::new(Wire::default())).await;
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        sqlx::raw_sql(
            "CREATE TRIGGER deny_health BEFORE INSERT ON credential_model_states
            BEGIN SELECT RAISE(ABORT,'synthetic health failure'); END",
        )
        .execute(pool)
        .await
        .unwrap();
        test_support::origin(origin, async {
            let (status, body) = read(fixture.open("responses", create()).await).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body["status"], "completed");
            assert_eq!(fixture.remaining().await, 1);
            assert!(health(&fixture).await.is_empty());
            assert_eq!(fixture.audits(1).await[0].status, "completed");
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
