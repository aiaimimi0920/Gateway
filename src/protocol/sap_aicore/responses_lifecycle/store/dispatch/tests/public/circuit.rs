//! Exact generation trials are exclusive and only upstream evidence completes them.
use super::*;
use crate::routing::model_circuit::{CircuitAttempt, Outcome};

pub(super) fn trial(fixture: &Fixture) -> Arc<CircuitAttempt> {
    let candidate = fixture
        .state
        .route_config
        .snapshot()
        .resolve_candidates_with_constraint(Some(MODEL), None)
        .candidates
        .into_iter()
        .find(|candidate| candidate.payload.credential_id.as_deref() == Some("exact"))
        .unwrap();
    let model = candidate.upstream_model.as_deref().unwrap_or(MODEL);
    let attempt = crate::routing::admission::attempt(&fixture.state, &candidate, model);
    attempt.begin().unwrap();
    attempt.finish(Outcome::RateLimited(0));
    attempt
}

#[tokio::test]
async fn sap_aicore_stored_http_half_open_is_single_send_and_success_releases_trial() {
    Box::pin(async {
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let wire = Arc::new(Wire {
            pause_create: Some((started.clone(), release.clone())),
            ..Wire::default()
        });
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let attempt = trial(&fixture);
        test_support::origin(origin, async {
            let first = fixture.open("responses", create());
            tokio::pin!(first);
            tokio::select! {
                _ = started.notified() => {}
                response = &mut first => panic!("premature response: {}", response.status()),
            }
            assert_eq!(
                attempt.begin().unwrap_err().code.as_deref(),
                Some("model_circuit_open")
            );
            let second = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                read(fixture.open("responses", create()).await).await
            });
            let (status, body) = second.await.unwrap();
            // A fresh request is filtered during normal route resolution;
            // pinned continuations instead receive model_circuit_open.
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert!(body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("temporarily_unavailable"));
            assert_eq!(fixture.remaining().await, 1);
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
            release.notify_one();
            let (status, body) = read(first.await).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            attempt.begin().unwrap();
            attempt.finish(Outcome::Success);
            let audits = fixture.audits(2).await;
            assert_eq!(
                audits
                    .iter()
                    .filter(|a| a.route_trace.as_ref().unwrap()["routeAttemptCount"] == 0)
                    .count(),
                1
            );
            let denied = audits
                .iter()
                .find(|a| a.route_trace.as_ref().unwrap()["routeAttemptCount"] == 0)
                .unwrap();
            assert!(denied.provider_account_id.is_none());
            assert!(denied.route_trace.as_ref().unwrap()["realCredentialRef"].is_null());
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_http_local_settlement_failure_does_not_poison_trial() {
    Box::pin(async {
        let (origin, fixture, server) = prepare(Arc::new(Wire::default())).await;
        let attempt = trial(&fixture);
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        sqlx::raw_sql("CREATE TRIGGER deny_settlement BEFORE UPDATE OF open ON gateway_sap_response_balance_holds
            BEGIN SELECT RAISE(ABORT,'synthetic settlement failure'); END").execute(pool).await.unwrap();
        test_support::origin(origin, async {
            let (status, body) = read(fixture.open("responses", create()).await).await;
            assert!(!status.is_success());
            assert_eq!(body["error"]["code"], "sap_aicore_response_settlement_pending");
            attempt.begin().unwrap();
            attempt.finish(Outcome::Success);
        }).await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    }).await;
}
