//! Bounded health storage and deadline/stale-selection regressions.
use super::*;
use crate::protocol::sap_aicore::responses_lifecycle::{
    evidence::Evidence,
    store::{run_submit, PinnedTarget},
};
use std::time::Duration;

async fn selected(fixture: &Fixture) -> (crate::auth::session::AuthenticatedSession, PinnedTarget) {
    let session = authenticate_request(
        &fixture.state,
        &AuthRequest {
            authorization: Some(format!("Bearer {}", fixture.token)),
            api_key: None,
            path: "/v1/responses".into(),
            method: "POST".into(),
        },
        None,
    )
    .await
    .unwrap();
    let target = PinnedTarget {
        public_model: MODEL.into(),
        provider: "sap_aicore-public".into(),
        credential: "exact".into(),
        upstream_model: "deployment1".into(),
        source_key: session.access_key_id.clone().unwrap(),
        platform_access: None,
    };
    (session, target)
}

#[tokio::test]
async fn sap_aicore_stored_stale_model_is_rejected_before_oauth_and_reservation() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let (session, mut target) = selected(&fixture).await;
        target.upstream_model = "stale-deployment".into();
        let observed = std::sync::atomic::AtomicBool::new(false);
        test_support::origin(origin, async {
            let req = crate::protocol::responses::normalize_responses(create()).unwrap();
            assert!(run_submit(
                &fixture.state,
                &session,
                &target,
                None,
                &req,
                1,
                SendHooks {
                    observe: &|_| observed.store(true, std::sync::atomic::Ordering::Relaxed),
                    ..SendHooks::default()
                }
            )
            .await
            .is_err());
            assert_eq!(fixture.remaining().await, 2);
            assert_eq!(
                wire.oauth_calls.load(std::sync::atomic::Ordering::Relaxed),
                0
            );
            assert!(wire.requests.lock().unwrap().is_empty());
            let local = fixture.state.local_runtime.as_ref().unwrap();
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM gateway_sap_response_receipts")
                    .fetch_one(&local.pool)
                    .await
                    .unwrap();
            assert_eq!(count, 0);
            assert!(!observed.load(std::sync::atomic::Ordering::Relaxed));
            assert!(local
                .list_model_states(crate::db::CredentialModelStateFilters::default())
                .await
                .unwrap()
                .is_empty());
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_body_deadline_retains_http_status_and_releases_trial_neutrally() {
    Box::pin(async {
        let wire = Arc::new(Wire {
            hold_body: true,
            ..Wire::default()
        });
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let trial = super::circuit::trial(&fixture);
        let (session, target) = selected(&fixture).await;
        let observation = parking_lot::Mutex::new(None);
        test_support::origin(origin, async {
            let req = crate::protocol::responses::normalize_responses(create()).unwrap();
            let hooks = SendHooks {
                observe: &|evidence| *observation.lock() = Some(evidence),
                deadline: Some(tokio::time::Instant::now() + std::time::Duration::from_secs(2)),
                ..SendHooks::default()
            };
            let error = run_submit(&fixture.state, &session, &target, None, &req, 1, hooks)
                .await
                .unwrap_err();
            assert_eq!(
                error.code.as_deref(),
                Some("sap_aicore_response_delivery_unknown")
            );
            match observation.lock().as_ref().unwrap() {
                Evidence::Failed { error, status } => {
                    assert_eq!(*status, Some(200));
                    assert!(crate::protocol::sap_aicore::errors::neutral(error));
                }
                _ => panic!("expected indeterminate delivery"),
            }
            // Neutral releases the lease but preserves the existing five-second trial backoff.
            assert_eq!(
                trial.begin().unwrap_err().code.as_deref(),
                Some("model_circuit_open")
            );
            tokio::time::timeout(Duration::from_secs(7), async {
                loop {
                    if trial.begin().is_ok() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .expect("neutral delivery must not leak the half-open lease");
            trial.finish(crate::routing::model_circuit::Outcome::Neutral);
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
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
async fn sap_aicore_stored_health_lock_timeout_preserves_receipt_and_releases_owner() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        *wire.rejection.lock().unwrap() = Some((503, b"private-error".to_vec()));
        let (origin, fixture, server) = prepare(wire.clone()).await;
        let lock = fixture.state.lifecycle.health_control.gate.lock().await;
        test_support::origin(origin, async {
            let started = std::time::Instant::now();
            let (status, body) = tokio::time::timeout(Duration::from_secs(9), async {
                read(fixture.open("responses", create()).await).await
            })
            .await
            .expect("health lock wait must be bounded");
            assert!(!status.is_success(), "{body}");
            assert!(started.elapsed() >= std::time::Duration::from_secs(5));
            assert_eq!(fixture.remaining().await, 1);
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
            assert_eq!(fixture.audits(1).await[0].upstream_status, Some(503));
            assert_eq!(
                Arc::strong_count(&fixture.state),
                1,
                "durable owner must exit while gate stays locked"
            );
        })
        .await;
        drop(lock);
        assert!(
            fixture
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_model_states(crate::db::CredentialModelStateFilters::default())
                .await
                .unwrap()
                .is_empty(),
            "cancelled health write must not complete later"
        );
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
