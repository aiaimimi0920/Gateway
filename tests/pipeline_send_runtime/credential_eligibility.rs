//! Mutable upstream eligibility overrides an already selected/sticky candidate.
use super::fixture::{authorize_candidates, candidate, context, TestState, Upstream, JSON_REPLY};
use axum::http::StatusCode;
use neuro_gateway::{pipeline::stage_send, routing::config::RouteConfigStore};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn disabled_credential_in_old_queue_is_never_sent() {
    let disabled = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let next = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let mut fixture = TestState::new();
    let mut stale = candidate(&disabled, "disabled");
    stale.payload.credential_id = Some("disabled-key".into());
    let document = serde_json::from_value(json!({"providers": [{
        "id": stale.provider_account_id, "preset": "openai", "base_url": disabled.base_url,
        "credentials": [{"id":"disabled-key", "api_key":"send-test-token", "enabled":false}]
    }]}))
    .unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().route_config =
        Arc::new(RouteConfigStore::from_document(document).unwrap());
    let mut ctx = context(false, vec![stale.clone(), candidate(&next, "next")]);
    authorize_candidates(&mut ctx);
    let result = stage_send::run(&mut ctx, &fixture.state).await;
    let counts = (disabled.requests().len(), next.requests().len());
    let mut exhausted = context(false, vec![stale]);
    authorize_candidates(&mut exhausted);
    let error = stage_send::run(&mut exhausted, &fixture.state)
        .await
        .err()
        .unwrap();
    disabled.finish().await;
    next.finish().await;
    fixture.finish().await;
    assert!(result.is_ok());
    assert_eq!(counts, (0, 1));
    assert_eq!(
        error.code.as_deref(),
        Some("upstream_credential_unavailable")
    );
    assert_eq!(error.http_status, Some(503));
}

#[tokio::test]
async fn upstream_disabled_key_is_persisted_before_fallback() {
    let rejected = Upstream::start(
        StatusCode::UNAUTHORIZED,
        false,
        r#"{"error":{"message":"API key is disabled"}}"#,
    )
    .await;
    let next = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::with_local_limits().await;
    let mut first = candidate(&rejected, "rejected");
    first.payload.credential_id = Some("revoked-key".into());
    let candidates = vec![first, candidate(&next, "next")];
    let mut results = Vec::new();
    for _ in 0..2 {
        let mut ctx = context(false, candidates.clone());
        authorize_candidates(&mut ctx);
        results.push(stage_send::run(&mut ctx, &fixture.state).await.is_ok());
    }
    let counts = (rejected.requests().len(), next.requests().len());
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
    assert_eq!(results, vec![true, true]);
    assert_eq!(counts, (1, 2));
}
