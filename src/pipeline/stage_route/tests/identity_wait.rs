//! Isolated route -> identity publication -> send admission, never a live migration.
use super::*;
use crate::routing::model_circuit::{CircuitAttempt, CircuitKey, Outcome};
use serde_json::json;

fn document(provider: &str) -> crate::routing::config::RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{
        "id":provider,"base_url":"http://127.0.0.1:1/v1",
        "supported_models":["gpt-4o"],"credentials":[{"id":"stable-account","api_key":"synthetic-only"}],
        "request_wait":{"mode":"disabled"}
    }]})).unwrap()
}

#[tokio::test]
async fn rename_between_routing_and_send_refreshes_the_account_before_any_io() {
    check_rename(false).await;
}

#[tokio::test]
async fn refreshed_identity_rejects_conflicting_wait_owners_before_any_io() {
    check_rename(true).await;
}

async fn check_rename(conflict: bool) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-identity-before-send-{}",
        uuid::Uuid::new_v4()
    ));
    let local = crate::local_runtime::LocalRuntime::open(&root)
        .await
        .unwrap();
    let mut state =
        make_state_with_route_store(RouteConfigStore::from_document(document("old")).unwrap());
    Arc::get_mut(&mut state).unwrap().local_runtime = Some(local);
    let mut ctx = PipelineContext::new(make_request(), None);
    run(&mut ctx, &state).await.unwrap();
    assert_eq!(ctx.candidates.len(), 1);
    assert_eq!(ctx.candidates[0].provider_account_id, "old");
    // Both queues exist before the newer alias snapshot is published. Candidate
    // refresh itself must coordinate it, not rely on an earlier wait-loop check.
    let leases: Vec<_> = if conflict {
        ["old", "new-api"]
            .into_iter()
            .map(|id| {
                state
                    .lifecycle
                    .request_waiters
                    .enter(id.into(), id.into(), 1)
                    .unwrap()
            })
            .collect()
    } else {
        vec![]
    };
    ctx.route_policy_config = Some(crate::db::GatewayRoutePolicyConfig {
        allowed_provider_account_ids: Some(vec!["old".into()]),
        ..Default::default()
    });
    // A synthetic earlier attempt must remain auditable under its observed name.
    ctx.attempted_provider_ids.lock().push("old".into());
    sqlx::query("INSERT INTO model_provider_aliases(alias,canonical) VALUES ('old','new-api')")
        .execute(&state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    state
        .route_config
        .replace_document(document("new-api"))
        .unwrap();
    let runtime = &state.lifecycle.model_circuits;
    runtime.enable_network_test_guard();
    for _ in 0..3 {
        let attempt = CircuitAttempt::new(
            runtime.clone(),
            vec![CircuitKey {
                provider: "fixture".into(),
                group: "default".into(),
                model: "gpt-4o".into(),
            }],
        );
        attempt.begin().unwrap();
        attempt.finish(Outcome::TransportFailure(0));
    }
    let error = Box::pin(crate::pipeline::stage_send::run(&mut ctx, &state))
        .await
        .err()
        .unwrap();
    assert_eq!(ctx.route_attempt_count(), 0);
    if conflict {
        assert_eq!(
            error.code.as_deref(),
            Some("recovery_wait_identity_conflict")
        );
        assert_eq!(ctx.candidates[0].provider_account_id, "old");
    } else {
        assert_eq!(error.code.as_deref(), Some("gateway_network_unstable"));
        assert_eq!(ctx.candidates[0].provider_account_id, "new-api");
        assert_eq!(
            ctx.candidates[0].payload.credential_id.as_deref(),
            Some("stable-account")
        );
        assert!(ctx.provider_allowed_for_send("new-api"));
        assert!(!ctx.has_other_provider_attempt("new-api"));
        assert_eq!(*ctx.attempted_provider_ids.lock(), vec!["old"]);
        assert_eq!(
            ctx.cash_route_snapshot
                .as_ref()
                .unwrap()
                .document()
                .providers[0]
                .id,
            "old"
        );
        assert!(ctx.projected_access_candidates.is_empty());
    }
    drop(leases);
    drop(ctx);
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
