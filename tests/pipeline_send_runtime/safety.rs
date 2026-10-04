//! Exact trusted account authorization and conservative side-effect replay limits.
use axum::http::StatusCode;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use neuro_gateway::protocol::canonical::{CanonicalTool, EndpointKind, MessageRole};
use serde_json::json;
use tokio::time::timeout;

use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY,
};

#[tokio::test]
async fn no_explicit_allowlist_never_promotes_a_failed_proxy_to_another_account() {
    for ids in [None, Some(Vec::new()), Some(vec!["  ".to_string()])] {
        let failed = Upstream::start(
            StatusCode::INTERNAL_SERVER_ERROR,
            false,
            r#"{"error":{"message":"proxy fixture failed"}}"#,
        )
        .await;
        let official = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
        let fixture = TestState::new();
        let first = candidate(&failed, "proxy");
        // The label/profile deliberately cannot serve as authorization.
        let mut second = candidate(&official, "official-paid-fixture");
        second.protocol_profile = "chatgpt_official".to_string();
        let mut ctx = context(true, vec![first, second]);
        authorize_candidates(&mut ctx);
        ctx.route_policy_config
            .as_mut()
            .unwrap()
            .allowed_provider_account_ids = ids;
        let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .err()
            .unwrap();
        assert_eq!(
            error.code.as_deref(),
            Some("provider_fallback_not_authorized")
        );
        assert_eq!(failed.requests().len(), 1);
        assert!(official.requests().is_empty());
        assert_eq!(ctx.route_attempt_count(), 1);
        assert!(ctx
            .request_budget
            .last_upstream_error()
            .unwrap()
            .message
            .contains("proxy fixture failed"));
        failed.finish().await;
        official.finish().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn policy_denied_candidate_has_zero_requests_even_when_first_in_queue() {
    let denied = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let allowed = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let first = candidate(&denied, "denied-official");
    let second = candidate(&allowed, "allowed-proxy");
    let mut ctx = context(false, vec![first, second.clone()]);
    authorize_candidates(&mut ctx);
    ctx.route_policy_config
        .as_mut()
        .unwrap()
        .allowed_provider_account_ids = Some(vec![format!(" {} ", second.provider_account_id)]);
    assert!(matches!(
        timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap(),
        PipelineOutput::Json(_)
    ));
    assert!(denied.requests().is_empty());
    assert_eq!(allowed.requests().len(), 1);
    assert_eq!(ctx.route_attempt_count(), 1);
    denied.finish().await;
    allowed.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn account_ids_are_case_sensitive_and_all_denied_returns_explicit_error() {
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let selected = candidate(&upstream, "exact-id");
    let mut ctx = context(false, vec![selected.clone()]);
    authorize_candidates(&mut ctx);
    ctx.route_policy_config
        .as_mut()
        .unwrap()
        .allowed_provider_account_ids = Some(vec![selected.provider_account_id.to_uppercase()]);
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.code.as_deref(), Some("provider_not_authorized"));
    assert_eq!(error.http_status, Some(403));
    assert_eq!(ctx.route_attempt_count(), 0);
    assert!(upstream.requests().is_empty());
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn tool_or_media_request_never_retries_or_creates_on_a_second_account() {
    for kind in ["tools", "tool_choice", "tool_history", "media"] {
        let failed = Upstream::start(
            StatusCode::SERVICE_UNAVAILABLE,
            false,
            r#"{"error":{"message":"side-effect fixture unavailable"}}"#,
        )
        .await;
        let untouched = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
        let fixture = TestState::new();
        let mut ctx = context(
            false,
            vec![
                candidate(&failed, "side-effect-first"),
                candidate(&untouched, "side-effect-second"),
            ],
        );
        authorize_candidates(&mut ctx);
        match kind {
            "tools" => ctx.canonical_req.tools.push(CanonicalTool {
                tool_type: "function".to_string(),
                name: Some("fixture_tool".to_string()),
                description: None,
                input_schema: Some(json!({"type":"object"})),
                raw: Default::default(),
            }),
            "tool_choice" => ctx.canonical_req.tool_choice = Some(json!("auto")),
            "tool_history" => ctx.canonical_req.messages[0].role = MessageRole::Tool,
            "media" => {
                ctx.canonical_req.endpoint_kind = EndpointKind::ImagesGenerations;
                ctx.canonical_req.raw_body = json!({"prompt":"local media fixture"});
            }
            _ => unreachable!(),
        }
        let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .err()
            .unwrap();
        assert_eq!(
            error.request_budget_stop_reason(),
            Some("attempt_limit"),
            "{kind}"
        );
        assert_eq!(failed.requests().len(), 1, "{kind}");
        assert!(untouched.requests().is_empty(), "{kind}");
        assert_eq!(ctx.route_attempt_count(), 1, "{kind}");
        failed.finish().await;
        untouched.finish().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn buffered_success_handoff_prevents_a_second_network_call() {
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(false, vec![candidate(&upstream, "buffered-handoff")]);
    assert!(stage_send::run(&mut ctx, &fixture.state).await.is_ok());
    let error = stage_send::run(&mut ctx, &fixture.state)
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.request_budget_stop_reason(),
        Some("response_handed_off")
    );
    assert_eq!(upstream.requests().len(), 1);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn standalone_explicit_route_authority_preserves_cross_proxy_fallback() {
    use neuro_gateway::routing::config::RouteConfigYaml;
    let failed = Upstream::start(
        StatusCode::INTERNAL_SERVER_ERROR,
        true,
        r#"{"error":{"message":"explicit route fixture failed"}}"#,
    )
    .await;
    let success = Upstream::start(StatusCode::OK, true, super::fixture::SSE_REPLY).await;
    let fixture = TestState::new();
    let doc: RouteConfigYaml = serde_json::from_value(json!({
        "providers":[
            {"id":"explicit-first","base_url":failed.base_url,"api_key":"send-test-token"},
            {"id":"explicit-second","base_url":success.base_url,"api_key":"send-test-token"}
        ],
        "model_routes":[{"pattern":"caller-alias","provider_ids":["explicit-first","explicit-second"]}]
    })).unwrap();
    fixture.state.route_config.replace_document(doc).unwrap();
    let resolution = fixture
        .state
        .route_config
        .snapshot()
        .resolve_candidates_with_authorization_for_account_group(Some("caller-alias"), None)
        .unwrap();
    let mut ctx = context(true, resolution.candidates);
    ctx.explicit_fallback_provider_ids = resolution.explicit_provider_ids.unwrap();
    let PipelineOutput::Sse(mut stream) =
        timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("expected fallback SSE")
    };
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        chunk.unwrap();
    }
    assert_eq!(failed.requests().len(), 1);
    assert_eq!(success.requests().len(), 1);
    assert_eq!(ctx.route_attempt_count(), 2);
    drop(stream);
    failed.finish().await;
    success.finish().await;
    fixture.finish().await;
}
