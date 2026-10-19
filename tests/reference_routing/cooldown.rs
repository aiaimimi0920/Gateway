use axum::http::StatusCode;
use neuro_gateway::db::CredentialModelStateFilters;
use neuro_gateway::pipeline::{stage_finalize, stage_send};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::time::timeout;

use super::fixture::{candidate, context, TestState, Upstream, DEADLINE};

#[tokio::test]
async fn upstream_explicit_retry_after_survives_send_and_durable_finalization() {
    let upstream = Upstream::start(
        StatusCode::TOO_MANY_REQUESTS,
        false,
        r#"{"error":{"message":"rate limit","retry_after":172800}}"#,
    )
    .await;
    let fixture = TestState::with_local_limits().await;
    let mut ctx = context(false, vec![candidate(&upstream, "hint")]);
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .expect("upstream rejects request");
    assert_eq!(error.http_status, Some(429));
    assert_eq!(
        upstream.requests().len(),
        1,
        "long hints must not be clipped"
    );
    stage_finalize::run_failure(&error, &ctx, &fixture.state).await;
    let rows = fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(CredentialModelStateFilters::default())
        .await
        .unwrap();
    upstream.finish().await;
    fixture.finish().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].failure_count, 1,
        "dispatch and finalize must not double-count"
    );
    let until =
        OffsetDateTime::parse(rows[0].cooldown_until.as_deref().unwrap(), &Rfc3339).unwrap();
    let remaining = (until - OffsetDateTime::now_utc()).whole_seconds();
    assert!(
        (172_795..=172_800).contains(&remaining),
        "explicit 48h hint lost: {remaining}"
    );
    assert_eq!(rows[0].model, "upstream-model");
}

#[tokio::test]
async fn sticky_provider_does_not_promote_a_cooling_candidate() {
    let upstream = Upstream::start(StatusCode::OK, false, "{}").await;
    let healthy = candidate(&upstream, "healthy");
    let mut cooling = candidate(&upstream, "cooling");
    cooling.cooldown_until = Some("9999-12-31T23:59:59Z".into());
    let queue = neuro_gateway::routing::queue::build_candidate_queue(
        &[healthy.clone(), cooling.clone()],
        "priority_weighted",
        Some(&cooling.provider_account_id),
        &Default::default(),
        None,
    );
    assert_eq!(queue[0].provider_account_id, healthy.provider_account_id);
    upstream.finish().await;
}

#[tokio::test]
async fn local_cooldown_writer_reader_and_candidate_contract() {
    let upstream = Upstream::start(StatusCode::OK, false, "{}").await;
    let fixture = TestState::with_local_limits().await;
    use futures::FutureExt;
    let outcome = std::panic::AssertUnwindSafe(super::health_store::contract(
        super::health_store::HealthStore::Local(fixture.state.local_runtime.as_ref().unwrap()),
        candidate(&upstream, "local-model"),
    ))
    .catch_unwind()
    .await;
    upstream.finish().await;
    fixture.finish().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

#[tokio::test]
async fn a_failed_candidate_keeps_its_hint_when_an_authorized_fallback_succeeds() {
    let limited = Upstream::start(
        StatusCode::TOO_MANY_REQUESTS,
        false,
        r#"{"error":{"message":"rate limit","retry_after":172800}}"#,
    )
    .await;
    let success = Upstream::start(StatusCode::OK, false, super::fixture::JSON_REPLY).await;
    let fixture = TestState::with_local_limits().await;
    let first = candidate(&limited, "limited");
    let second = candidate(&success, "fallback");
    let mut ctx = context(false, vec![first.clone(), second.clone()]);
    super::fixture::authorize_candidates(&mut ctx);
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        output,
        neuro_gateway::pipeline::PipelineOutput::Json(_)
    ));
    let rows = fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(CredentialModelStateFilters::default())
        .await
        .unwrap();
    limited.finish().await;
    success.finish().await;
    fixture.finish().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].provider_account_id, first.provider_account_id);
    assert_ne!(rows[0].provider_account_id, second.provider_account_id);
    assert_eq!(rows[0].failure_count, 1);
    let until =
        OffsetDateTime::parse(rows[0].cooldown_until.as_deref().unwrap(), &Rfc3339).unwrap();
    let remaining = (until - OffsetDateTime::now_utc()).whole_seconds();
    assert!(
        (172_795..=172_800).contains(&remaining),
        "fallback lost the explicit hint: {remaining}"
    );
}
