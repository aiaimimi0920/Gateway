//! Internal sender tests complement public-router tests; routing requires the SDK feature and explicit enable.
use super::*;
use crate::pipeline::copilot_identity::CopilotIdentity;
use crate::routing::candidate::RouteCandidate;
use serde_json::json;
#[path = "../copilot_identity/fixture.rs"]
mod fixture;
#[path = "copilot_tests/prepaid.rs"]
mod prepaid;
#[path = "copilot_tests/public.rs"]
mod public;
#[path = "copilot_tests/public_enabled.rs"]
mod public_enabled;
#[path = "copilot_tests/termination.rs"]
mod termination;

async fn balance_snapshot(f: &fixture::Fixture) -> serde_json::Value {
    let mut value = serde_json::to_value(f.local.access_balance(&f.key).await.unwrap()).unwrap();
    // Reserve/refund may touch timestamps without changing purchased credit or remaining balance.
    if let Some(balance) = value.as_object_mut() {
        balance.remove("updatedAt");
    }
    value
}

#[test]
fn copilot_sender_http_missing_status_remains_unobserved() {
    assert_eq!(buffered::upstream_status_observation(None, None), None);
    assert_eq!(
        buffered::upstream_status_observation(Some("oauth"), None),
        None
    );
    assert_eq!(
        buffered::upstream_status_observation(Some("copilot_sdk"), None),
        Some(None)
    );
    assert_eq!(
        buffered::upstream_status_observation(None, Some(201)),
        Some(Some(201))
    );
}

async fn prepared(f: &fixture::Fixture) -> PreparedAttempt {
    let controller = f
        .console
        .state
        .concurrency_registry
        .get_or_create("copilot");
    let permit = controller.acquire().await;
    PreparedAttempt {
        route_policy_config: None,
        provider_attempt_gate: super::super::stage_rate_limit::provider_attempt_gate(
            &f.ctx,
            &f.console.state,
            "copilot",
        )
        .unwrap()
        .with_audit_candidate(&f.candidate, "synthetic-model"),
        model: "synthetic-model".into(),
        reply_model: "public-alias".into(),
        effective_payload: f.candidate.payload.clone(),
        retry_policy: RetryPolicy::default(),
        original_tools: vec![],
        original_tool_choice: None,
        original_messages_text: f.ctx.canonical_req.messages_text(),
        tools_were_injected: false,
        controller,
        permit,
    }
}

async fn finish(f: fixture::Fixture) {
    f.console
        .state
        .shutdown
        .request("Copilot sender fixture finished");
    tokio::time::timeout(
        std::time::Duration::from_secs(45),
        f.console.state.lifecycle.background_tasks.join(),
    )
    .await
    .unwrap()
    .unwrap();
    f.close().await;
}

async fn configured() -> fixture::Fixture {
    let mut f = fixture::Fixture::new().await;
    f.replace(|doc| {
        doc.providers[0].credentials[0].copilot_github_token = Some("synthetic-token".into())
    });
    f.ctx.cash_route_snapshot = Some(f.console.state.route_config.snapshot());
    f.ctx.canonical_req = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"public-alias", "messages":[{"role":"user","content":"ok"}]
    }))
    .unwrap();
    f.local
        .save_access_key(
            Some(&f.key),
            fixture::input(json!({
                "scopes":["relay"], "models":["public-alias"], "providerIds":["copilot"]
            })),
        )
        .await
        .unwrap();
    f
}

#[tokio::test]
#[ignore = "requires synthetic GATEWAY_COPILOT_CLI, workspace root and GATEWAY_PROBE_ROOT; run serially"]
async fn copilot_sender_chat_and_responses_persist_unknown_status_and_usage() {
    for responses in [false, true] {
        let mut f = configured().await;
        if responses {
            f.ctx.canonical_req = crate::protocol::responses::normalize_responses(json!({
                "model":"public-alias", "input":"known-usage"
            }))
            .unwrap();
        }
        let before = balance_snapshot(&f).await;
        super::super::stage_finalize::begin_request_audit(&mut f.ctx, &f.console.state).await;
        let attempt = prepared(&f).await;
        let result =
            recovery_wait::send_cancellable(&mut f.ctx, &f.console.state, &f.candidate, 0, attempt)
                .await;
        let body = match result {
            Ok(PipelineOutput::Json(body)) => body,
            Err(AttemptError::Next(error) | AttemptError::Stop(error)) => {
                panic!("Copilot failed: {error}")
            }
            Ok(_) => panic!("expected buffered Copilot JSON"),
        };
        assert_eq!(body["model"], "public-alias");
        assert!(body.get("usage").is_none());
        assert!(matches!(f.ctx.observed_usage, Some(None)));
        assert_eq!(f.ctx.observed_upstream_status, Some(None));
        assert_eq!(f.ctx.route_attempt_count(), 1);
        assert_eq!(f.ctx.audited_provider_id().as_deref(), Some("copilot"));
        assert_eq!(
            f.ctx.audited_real_credential_ref().as_deref(),
            Some("account")
        );
        super::super::stage_finalize::run(Some(&body), &f.ctx, &f.console.state).await;
        let rows = f
            .local
            .list_audits(&crate::db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.status, "completed");
        assert!(row.upstream_status.is_none());
        assert!(
            row.prompt_tokens.is_none()
                && row.completion_tokens.is_none()
                && row.total_tokens.is_none()
        );
        assert_eq!(row.route_trace.as_ref().unwrap()["routeAttemptCount"], 1);
        assert_eq!(balance_snapshot(&f).await, before);
        assert!(!serde_json::to_string(&rows)
            .unwrap()
            .contains("synthetic-token"));
        assert_eq!(
            f.console
                .state
                .route_config
                .snapshot()
                .resolve_candidates(Some("public-alias"))
                .len(),
            1
        );
        finish(f).await;
    }
}

#[tokio::test]
async fn copilot_sender_audit_preserves_http_status_and_legacy_default() {
    for (observed, expected) in [
        (None, Some(200)),
        (Some(Some(201)), Some(201)),
        (Some(None), None),
    ] {
        let mut f = fixture::Fixture::new().await;
        f.ctx.observed_upstream_status = observed;
        f.ctx.observed_usage = Some(None);
        super::super::stage_finalize::begin_request_audit(&mut f.ctx, &f.console.state).await;
        super::super::stage_finalize::run(
            Some(&json!({"id":"synthetic-response"})),
            &f.ctx,
            &f.console.state,
        )
        .await;
        let rows = f
            .local
            .list_audits(&crate::db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].upstream_status, expected);
        assert!(rows[0].total_tokens.is_none());
        finish(f).await;
    }
}
