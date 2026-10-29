//! Real SQLite health plus normal admission; no external hosts or host Redis are contacted.
use super::*;
use crate::routing::model_circuit::{health_control, CircuitAttempt, CircuitKey, Outcome};

async fn fixture() -> (Arc<AppState>, std::path::PathBuf) {
    let document = serde_json::from_value(serde_json::json!({"providers":[
        {"id":"site","preset":"openai","base_url":"http://127.0.0.1:1/v1","api_key":"synthetic","supported_models":["gpt-4o"]},
        {"id":"other","preset":"openai","base_url":"http://127.0.0.1:1/v1","api_key":"other-synthetic","supported_models":["gpt-4o"]}
    ],"model_routes":[{"pattern":"gpt-4o","provider_ids":["site","other"]}],"aliases":{}}))
    .unwrap();
    let root = std::env::temp_dir().join(format!(
        "gateway-local-egress-health-{}",
        uuid::Uuid::new_v4()
    ));
    let local = crate::local_runtime::LocalRuntime::open(&root)
        .await
        .unwrap();
    let mut state = make_state_with_route_store(RouteConfigStore::from_document(document).unwrap());
    Arc::get_mut(&mut state).unwrap().local_runtime = Some(local);
    (state, root)
}
fn failure(
    provider: &str,
    model: &str,
    status: Option<u16>,
) -> db::RecordCredentialModelFailureInput {
    db::RecordCredentialModelFailureInput {
        provider_account_id: provider.into(),
        provider_credential_id: None,
        provider_credential_ref: None,
        protocol_profile: Some("openai".into()),
        model: model.into(),
        upstream_status: status,
        upstream_retry_after_ms: None,
        error_message: Some(
            if status.is_some() {
                "invalid API key fixture"
            } else {
                "Network error: synthetic disconnect"
            }
            .into(),
        ),
        classification: crate::provider_failure::classify_provider_failure(
            status,
            None,
            Some(if status.is_some() {
                "invalid API key fixture"
            } else {
                "Network error: synthetic disconnect"
            }),
        ),
    }
}
fn trip(state: &AppState) -> u64 {
    let runtime = &state.lifecycle.model_circuits;
    runtime.enable_network_test_guard();
    for _ in 0..3 {
        let attempt = CircuitAttempt::new(
            runtime.clone(),
            vec![CircuitKey {
                provider: "site".into(),
                group: "default".into(),
                model: "gpt-4o".into(),
            }],
        );
        attempt.begin().unwrap();
        attempt.finish(Outcome::TransportFailure(0));
    }
    runtime.network_epoch().unwrap()
}
async fn close(state: Arc<AppState>, root: std::path::PathBuf) {
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
fn set_wait_policy(state: &AppState, mode: &str) {
    let mut document = state.route_config.snapshot().document().clone();
    for provider in &mut document.providers {
        provider.request_wait = Some(crate::request_wait::RequestWaitPolicy {
            mode: if mode == "disabled" {
                crate::request_wait::RequestWaitMode::Disabled
            } else {
                crate::request_wait::RequestWaitMode::Bounded
            },
            total_timeout_seconds: Some(1),
        });
    }
    state.route_config.replace_document(document).unwrap();
}
#[tokio::test]
async fn egress_rollback_restores_persisted_baseline_and_keeps_real_auth_refusal() {
    let (state, root) = fixture().await;
    set_wait_policy(&state, "disabled");
    let local = state.local_runtime.as_ref().unwrap();
    let identity = db::RecordCredentialModelSuccessInput {
        provider_account_id: "site".into(),
        provider_credential_id: None,
        provider_credential_ref: None,
        protocol_profile: Some("openai".into()),
        model: "gpt-4o".into(),
    };
    local.model_success(identity).await.unwrap();
    let before = local.list_model_states(Default::default()).await.unwrap()[0].clone();
    let epoch = trip(&state);
    for _ in 0..3 {
        health_control::record_failure(&state, failure("site", "gpt-4o", None), None)
            .await
            .unwrap();
    }
    health_control::record_failure(&state, failure("other", "gpt-4o", Some(401)), None)
        .await
        .unwrap();
    health_control::complete_probe(&state, epoch, false)
        .await
        .unwrap();
    let mut ctx = PipelineContext::new(make_request(), None);
    run(&mut ctx, &state).await.unwrap();
    // Routing retains authorized candidates; the send gate must refuse before IO.
    assert!(!ctx.candidates.is_empty());
    let error = Box::pin(crate::pipeline::stage_send::run(&mut ctx, &state))
        .await
        .err()
        .unwrap();
    assert_eq!(error.code.as_deref(), Some("gateway_network_unstable"));
    assert_eq!(ctx.route_attempt_count(), 0);
    assert_eq!(
        local
            .list_model_states(Default::default())
            .await
            .unwrap()
            .iter()
            .find(|r| r.provider_account_id == "site")
            .unwrap()
            .failure_count,
        3
    );
    health_control::complete_probe(&state, epoch, true)
        .await
        .unwrap();
    let rows = local.list_model_states(Default::default()).await.unwrap();
    let restored = rows
        .iter()
        .find(|r| r.provider_account_id == "site")
        .unwrap();
    assert_eq!(restored.status, "active");
    assert_eq!(restored.failure_count, before.failure_count);
    assert_eq!(restored.cooldown_until, before.cooldown_until);
    assert_eq!(restored.last_success_at, before.last_success_at);
    assert!(restored.updated_at > before.updated_at);
    assert_eq!(
        rows.iter()
            .find(|r| r.provider_account_id == "other")
            .unwrap()
            .status,
        "blocked"
    );
    close(state, root).await;
}

#[tokio::test]
async fn egress_waiting_preserves_network_error_and_deadline_without_a_send() {
    let (state, root) = fixture().await;
    set_wait_policy(&state, "bounded");
    trip(&state);
    let mut ctx = PipelineContext::new(make_request(), None);
    run(&mut ctx, &state).await.unwrap();
    assert!(!ctx.candidates.is_empty());
    for candidate in &ctx.candidates {
        let error =
            crate::routing::admission::check_candidate(&state, candidate, "gpt-4o").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("gateway_network_unstable"));
    }
    // SQLite identity reads use real worker IO before wait-budget configuration;
    // paused time may jump through pool timeouts before that IO can complete.
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        Box::pin(crate::pipeline::stage_send::run(&mut ctx, &state)),
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(error.request_budget_stop_reason(), Some("deadline"));
    assert_eq!(ctx.route_attempt_count(), 0);
    close(state, root).await;
}

#[tokio::test]
async fn egress_outage_does_not_authorize_unknown_groups_or_create_missing_routes() {
    let (state, root) = fixture().await;
    set_wait_policy(&state, "disabled");
    trip(&state);
    let mut ctx = PipelineContext::new(make_request(), None);
    ctx.account_group_id = Some("not-an-authorized-group".into());
    let error = run(&mut ctx, &state).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("account_group_not_found"));
    assert!(ctx.candidates.is_empty());
    state
        .route_config
        .replace_document(serde_json::from_value(serde_json::json!({"providers":[]})).unwrap())
        .unwrap();
    let mut ctx = PipelineContext::new(make_request(), None);
    let error = run(&mut ctx, &state).await.unwrap_err();
    assert_ne!(error.code.as_deref(), Some("gateway_network_unstable"));
    assert!(ctx.candidates.is_empty());
    assert_eq!(ctx.route_attempt_count(), 0);
    close(state, root).await;
}
#[tokio::test]
async fn newer_auth_failure_is_not_replaced_by_old_network_checkpoint() {
    let (state, root) = fixture().await;
    let epoch = trip(&state);
    health_control::record_failure(&state, failure("site", "gpt-4o", None), None)
        .await
        .unwrap();
    health_control::complete_probe(&state, epoch, false)
        .await
        .unwrap();
    health_control::record_failure(&state, failure("site", "gpt-4o", Some(401)), None)
        .await
        .unwrap();
    health_control::complete_probe(&state, epoch, true)
        .await
        .unwrap();
    let rows = state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(rows[0].status, "blocked");
    close(state, root).await;
}
#[tokio::test]
async fn manual_reset_ignores_old_request_feedback_and_never_changes_other_provider() {
    let (state, root) = fixture().await;
    let started = std::time::Instant::now();
    health_control::record_failure(&state, failure("site", "gpt-4o", Some(401)), Some(started))
        .await
        .unwrap();
    health_control::record_failure(&state, failure("other", "gpt-4o", Some(401)), Some(started))
        .await
        .unwrap();
    let before = state.route_config.snapshot().document().clone();
    let mut after = before.clone();
    // The provider card is an aggregate credential dispatch switch, not a provider.enabled field.
    after.providers[0].credentials = vec![serde_json::from_value(
        serde_json::json!({"id":"site::default","api_key":"synthetic","enabled":false}),
    )
    .unwrap()];
    state.route_config.replace_document(after.clone()).unwrap();
    health_control::apply_provider_toggles(&state, &before, &after)
        .await
        .unwrap();
    after.providers[0].credentials[0].enabled = Some(true);
    let disabled = state.route_config.snapshot().document().clone();
    state.route_config.replace_document(after.clone()).unwrap();
    health_control::apply_provider_toggles(&state, &disabled, &after)
        .await
        .unwrap();
    health_control::record_failure(&state, failure("site", "gpt-4o", Some(401)), Some(started))
        .await
        .unwrap();
    let rows = state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r.provider_account_id == "site")
            .unwrap()
            .status,
        "active"
    );
    assert_eq!(
        rows.iter()
            .find(|r| r.provider_account_id == "other")
            .unwrap()
            .status,
        "blocked"
    );
    close(state, root).await;
}

#[tokio::test]
async fn recovered_egress_rejects_late_transport_feedback_but_not_real_auth_failure() {
    let (state, root) = fixture().await;
    let started = std::time::Instant::now();
    let epoch = trip(&state);
    health_control::complete_probe(&state, epoch, false)
        .await
        .unwrap();
    let target = state
        .route_config
        .snapshot()
        .select_credential_probe_target("site::default")
        .unwrap();
    let (_, report) = crate::provider_runtime::probe_console_target(&state, &target).await;
    assert_eq!(
        report.status,
        crate::provider_runtime::ProviderPayloadProbeStatus::Unsupported
    );
    let sweep =
        crate::provider_credential_probe_scheduler::sweep_scheduled_provider_credentials_once(
            &state,
        )
        .await
        .unwrap();
    assert_eq!(sweep.due_count, 0);
    health_control::complete_probe(&state, epoch, true)
        .await
        .unwrap();
    health_control::record_failure(&state, failure("site", "gpt-4o", None), Some(started))
        .await
        .unwrap();
    assert!(state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(Default::default())
        .await
        .unwrap()
        .is_empty());
    health_control::record_failure(&state, failure("site", "gpt-4o", Some(401)), Some(started))
        .await
        .unwrap();
    assert_eq!(
        state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_model_states(Default::default())
            .await
            .unwrap()[0]
            .status,
        "blocked"
    );
    close(state, root).await;
}

#[tokio::test]
async fn recovery_cas_never_overwrites_new_external_failure() {
    let (state, root) = fixture().await;
    let epoch = trip(&state);
    health_control::complete_probe(&state, epoch, false)
        .await
        .unwrap();
    health_control::record_failure(&state, failure("site", "gpt-4o", None), None)
        .await
        .unwrap();
    let local = state.local_runtime.as_ref().unwrap();
    local
        .model_failure(failure("site", "gpt-4o", Some(401)))
        .await
        .unwrap();
    health_control::complete_probe(&state, epoch, true)
        .await
        .unwrap();
    assert_eq!(
        local.list_model_states(Default::default()).await.unwrap()[0].status,
        "blocked"
    );
    close(state, root).await;
}

#[tokio::test]
async fn failure_writer_cas_protects_auth_inserted_between_checkpoint_capture_and_write() {
    let (state, root) = fixture().await;
    let local = state.local_runtime.as_ref().unwrap();
    let input = failure("site", "gpt-4o", None);
    let id = db::provider_credential_model_state_id("site", None, None, Some("openai"), "gpt-4o");
    let before = local.read_health_row(&id).await.unwrap();
    assert!(before.is_none());
    local
        .model_failure(failure("site", "gpt-4o", Some(401)))
        .await
        .unwrap();
    assert!(local
        .record_window_failure(input, before.as_ref())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        local.list_model_states(Default::default()).await.unwrap()[0].status,
        "blocked"
    );
    close(state, root).await;
}

#[tokio::test]
async fn outage_does_not_undo_balance_rejection_without_http_status() {
    let (state, root) = fixture().await;
    let epoch = trip(&state);
    health_control::complete_probe(&state, epoch, false)
        .await
        .unwrap();
    let mut quota = failure("other", "gpt-4o", None);
    quota.error_message = Some("quota exhausted".into());
    quota.classification = crate::provider_failure::classify_provider_failure(
        None,
        None,
        quota.error_message.as_deref(),
    );
    health_control::record_failure(&state, quota, None)
        .await
        .unwrap();
    health_control::complete_probe(&state, epoch, true)
        .await
        .unwrap();
    let rows = state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(rows[0].status, "cooling");
    assert_eq!(rows[0].failure_class.as_deref(), Some("quota_exhausted"));
    close(state, root).await;
}
