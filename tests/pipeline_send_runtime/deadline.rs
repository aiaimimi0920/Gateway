//! Real transport/queue cancellation and the send-start/recovery admission seam.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use neuro_gateway::error::GatewayError;
use neuro_gateway::pipeline::{request_budget::RequestBudget, stage_rate_limit, stage_send};
use neuro_gateway::retry::RetryPolicy;
use tokio::time::{advance, timeout};

use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY,
};

#[tokio::test]
async fn deadline_while_queued_has_zero_sends_and_preserves_other_permits() {
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "queue-deadline");
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut permits = Vec::new();
    for _ in 0..10 {
        permits.push(controller.acquire().await);
    }
    let mut ctx = context(false, vec![selected]);
    ctx.request_budget = RequestBudget::new(6, Duration::from_millis(100));
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.request_budget_stop_reason(), Some("deadline"));
    assert_eq!(ctx.route_attempt_count(), 0);
    assert!(upstream.requests().is_empty());
    assert_eq!(controller.snapshot().active_count, 10);
    drop(permits);
    assert_eq!(controller.snapshot().active_count, 0);
    assert!(stage_send::run(&mut ctx, &fixture.state).await.is_err());
    assert!(upstream.requests().is_empty());
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn deadline_in_flight_releases_permit_and_never_sends_again() {
    for streaming in [false, true] {
        let upstream = Upstream::start_pending(streaming).await;
        let fixture = TestState::new();
        let selected = candidate(&upstream, "in-flight-deadline");
        let controller = fixture
            .state
            .concurrency_registry
            .get_or_create(&selected.provider_account_id);
        let mut ctx = context(streaming, vec![selected]);
        ctx.request_budget = RequestBudget::new(6, Duration::from_millis(150));
        let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .err()
            .unwrap();
        assert_eq!(error.request_budget_stop_reason(), Some("deadline"));
        assert_eq!(upstream.requests().len(), 1);
        assert_eq!(ctx.route_attempt_count(), 1);
        assert_eq!(controller.snapshot().active_count, 0);
        assert_eq!(controller.snapshot().current_limit, 10);
        assert!(ctx.request_budget.last_upstream_error().is_none());
        assert!(stage_send::run(&mut ctx, &fixture.state).await.is_err());
        assert_eq!(upstream.requests().len(), 1);
        upstream.finish().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn deadline_during_s1_sleep_preserves_the_last_real_failure() {
    let failed = Upstream::start(
        StatusCode::SERVICE_UNAVAILABLE,
        false,
        r#"{"error":{"message":"sleep fixture unavailable"}}"#,
    )
    .await;
    let untouched = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        false,
        vec![
            candidate(&failed, "sleep-deadline"),
            candidate(&untouched, "sleep-untouched"),
        ],
    );
    authorize_candidates(&mut ctx);
    ctx.request_budget = RequestBudget::new(6, Duration::from_millis(150));
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.request_budget_stop_reason(), Some("deadline"));
    let original = ctx.request_budget.last_upstream_error().unwrap();
    assert_eq!(error.code, original.code);
    assert_eq!(error.message, original.message);
    assert!(error.message.contains("sleep fixture unavailable"));
    assert_eq!(failed.requests().len(), 1);
    assert!(untouched.requests().is_empty());
    failed.finish().await;
    untouched.finish().await;
    fixture.finish().await;
}

#[tokio::test(start_paused = true)]
async fn policy_tightens_from_original_start_and_never_resets_or_accepts_corruption() {
    let mut ctx = context(false, vec![]);
    let budget = ctx.request_budget.clone();
    advance(Duration::from_secs(2)).await;
    budget.configure(&ctx.canonical_req, Some(3)).unwrap();
    budget.configure(&ctx.canonical_req, Some(300)).unwrap();
    for seconds in [-1, 0, 301, i32::MAX] {
        let error = budget
            .configure(&ctx.canonical_req, Some(seconds))
            .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("invalid_request_budget_policy"));
    }
    advance(Duration::from_secs(1)).await;
    assert_eq!(
        budget
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("deadline")
    );
    assert_eq!(budget.attempts(), 0);
    ctx.canonical_req.tool_choice = Some(serde_json::json!("auto"));
    let budget = RequestBudget::new(6, Duration::from_secs(30));
    budget.configure(&ctx.canonical_req, None).unwrap();
    ctx.canonical_req.tool_choice = None;
    budget.configure(&ctx.canonical_req, None).unwrap();
    budget.begin_attempt().unwrap();
    assert!(budget.begin_attempt().is_err());
}

#[tokio::test]
async fn fresh_recovery_invocations_share_slots_and_admission_is_not_a_send() {
    let fixture = TestState::new();
    let mut ctx = context(false, vec![]);
    ctx.request_budget = RequestBudget::new(2, Duration::from_secs(30));
    let gate =
        stage_rate_limit::provider_attempt_gate(&ctx, &fixture.state, "fixture-provider").unwrap();
    gate.admit().await.unwrap();
    gate.clone().admit().await.unwrap();
    assert_eq!(ctx.route_attempt_count(), 0);
    let sends = AtomicUsize::new(0);
    let observations = AtomicUsize::new(0);
    let policy = RetryPolicy {
        max_retries: 0,
        ..Default::default()
    };
    let original =
        GatewayError::service_unavailable("recovery original error").with_code("recovery_code");
    let first: Result<(), GatewayError> = gate
        .execute_observed(
            || async {
                sends.fetch_add(1, Ordering::Relaxed);
                Err(original.clone())
            },
            |_| {
                observations.fetch_add(1, Ordering::Relaxed);
            },
            &policy,
        )
        .await;
    assert!(first.is_err());
    let recovery = gate.clone();
    recovery.admit().await.unwrap();
    recovery
        .execute_observed(
            || async {
                sends.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
            |_| {
                observations.fetch_add(1, Ordering::Relaxed);
            },
            &policy,
        )
        .await
        .unwrap();
    let error = recovery
        .execute_observed(
            || async {
                sends.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
            |_| {
                observations.fetch_add(1, Ordering::Relaxed);
            },
            &policy,
        )
        .await
        .unwrap_err();
    assert_eq!(error.request_budget_stop_reason(), Some("attempt_limit"));
    assert_eq!(error.code, original.code);
    assert_eq!(sends.load(Ordering::Relaxed), 2);
    assert_eq!(observations.load(Ordering::Relaxed), 2);
    assert_eq!(ctx.route_attempt_count(), 2);
    assert_eq!(*ctx.attempted_provider_ids.lock(), vec!["fixture-provider"]);
    fixture.finish().await;
}

#[tokio::test]
async fn actual_rate_limit_rejection_never_counts_a_send_or_overwrites_real_failure() {
    use neuro_gateway::auth::session::AuthenticatedSession;
    use neuro_gateway::db::{GatewayRateLimitDefinition, GatewayRoutePolicyConfig};
    use neuro_gateway::error::FallbackHint;
    let fixture = TestState::with_local_limits().await;
    let mut ctx = context(false, vec![]);
    ctx.session = Some(AuthenticatedSession {
        project_id: "limit-fixture-project".into(),
        tenant_id: "limit-fixture-tenant".into(),
        user_id: None,
        credential_ref: None,
        scopes: vec![],
        api_key_id: None,
        user_credential_id: None,
        access_key_id: None,
        access_key_kind: None,
    });
    ctx.route_policy_config = Some(GatewayRoutePolicyConfig {
        rate_limit_enforcement_version: Some("v1".into()),
        provider_attempt_rate_limit: Some(GatewayRateLimitDefinition {
            window_seconds: 60,
            max_requests: 1,
        }),
        ..Default::default()
    });
    let denied =
        stage_rate_limit::provider_attempt_gate(&ctx, &fixture.state, "denied-fixture").unwrap();
    denied.admit().await.unwrap();
    assert_eq!(denied.admit().await.unwrap_err().http_status, Some(429));
    assert_eq!(ctx.route_attempt_count(), 0);
    assert!(ctx.attempted_provider_ids.lock().is_empty());
    let retry =
        stage_rate_limit::provider_attempt_gate(&ctx, &fixture.state, "retry-fixture").unwrap();
    retry.admit().await.unwrap();
    let sends = AtomicUsize::new(0);
    let observations = AtomicUsize::new(0);
    let mut original =
        GatewayError::service_unavailable("real model failure").with_code("real_code");
    original.fallback_hint = FallbackHint::Retry {
        delay_ms: 0,
        reason: "fixture delay".into(),
    };
    let result: Result<(), GatewayError> = retry
        .execute_observed(
            || async {
                sends.fetch_add(1, Ordering::Relaxed);
                Err(original.clone())
            },
            |_| {
                observations.fetch_add(1, Ordering::Relaxed);
            },
            &RetryPolicy {
                max_retries: 1,
                ..Default::default()
            },
        )
        .await;
    assert_eq!(result.unwrap_err().http_status, Some(429));
    assert_eq!(sends.load(Ordering::Relaxed), 1);
    assert_eq!(observations.load(Ordering::Relaxed), 1);
    assert_eq!(ctx.route_attempt_count(), 1);
    assert_eq!(
        ctx.request_budget.last_upstream_error().unwrap().code,
        original.code
    );
    fixture.finish().await;
}
