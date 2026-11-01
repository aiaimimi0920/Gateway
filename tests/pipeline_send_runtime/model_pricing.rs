//! Pricing admission failures advance once, retaining authorization and request limits.
use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, JSON_REPLY, SSE_REPLY,
};
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineContext, PipelineOutput};
use neuro_gateway::protocol::canonical::{CanonicalTool, EndpointKind};
use tokio::time::timeout;

pub const PRICING_BODY: &str = r#"{"error":{"message":"model gpt-6.1-sol tiered expr run failed: expr compile error: unknown name cc12h (1:62)","type":"new_api_error","code":"model_price_error"}}"#;
pub const RESPONSES_REPLY: &str = "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n";

pub fn with_tools(ctx: &mut PipelineContext) {
    ctx.canonical_req.tools.push(CanonicalTool {
        tool_type: "function".into(),
        name: Some("fixture_tool".into()),
        description: None,
        input_schema: None,
        raw: Default::default(),
    });
}

#[tokio::test]
async fn model_pricing_400_falls_back_with_tools_in_both_protocols_and_modes() {
    for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Responses] {
        for streaming in [true, false] {
            let rejected = Upstream::start(StatusCode::BAD_REQUEST, false, PRICING_BODY).await;
            let responses = endpoint == EndpointKind::Responses;
            let next = Upstream::start(
                StatusCode::OK,
                streaming || responses,
                if responses {
                    RESPONSES_REPLY
                } else if streaming {
                    SSE_REPLY
                } else {
                    JSON_REPLY
                },
            )
            .await;
            let fixture = TestState::new();
            let mut ctx = context(
                streaming,
                vec![candidate(&rejected, "pricing"), candidate(&next, "healthy")],
            );
            ctx.canonical_req.endpoint_kind = endpoint.clone();
            if responses {
                for c in &mut ctx.candidates {
                    c.payload.responses_path = Some("/v1/responses".into());
                    c.payload.chat_completions_path = None;
                }
            }
            with_tools(&mut ctx);
            authorize_candidates(&mut ctx);
            let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
                .await
                .unwrap()
                .unwrap_or_else(|e| panic!("pricing refusal must advance before output: {e:?}"));
            if let PipelineOutput::Sse(stream) = output {
                let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
                assert!(chunks.iter().all(Result::is_ok));
            } else {
                assert!(!streaming);
            }
            assert_eq!((rejected.requests().len(), next.requests().len()), (1, 1));
            assert_eq!(ctx.route_attempt_count(), 2);
            assert_eq!(
                ctx.selected_provider_id.as_deref(),
                Some(ctx.candidates[1].provider_account_id.as_str())
            );
            rejected.finish().await;
            next.finish().await;
            fixture.finish().await;
        }
    }
}

#[tokio::test]
async fn model_pricing_400_does_not_bypass_fallback_authorization() {
    let rejected = Upstream::start(StatusCode::BAD_REQUEST, false, PRICING_BODY).await;
    let next = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        false,
        vec![
            candidate(&rejected, "pricing"),
            candidate(&next, "unauthorized"),
        ],
    );
    with_tools(&mut ctx);
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_fallback_not_authorized")
    );
    assert_eq!(rejected.requests().len(), 1);
    assert!(next.requests().is_empty());
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn model_pricing_refusals_share_six_attempt_limit() {
    let rejected = Upstream::start(StatusCode::BAD_REQUEST, false, PRICING_BODY).await;
    let next = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = TestState::new();
    let mut candidates: Vec<_> = (0..6)
        .map(|i| candidate(&rejected, &format!("pricing-{i}")))
        .collect();
    candidates.push(candidate(&next, "beyond-budget"));
    let mut ctx = context(false, candidates);
    with_tools(&mut ctx);
    authorize_candidates(&mut ctx);
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.request_budget_stop_reason(), Some("attempt_limit"));
    assert_eq!(error.code.as_deref(), Some("model_price_error"));
    assert_eq!(error.http_status, Some(400));
    assert!(error.message.contains("expr compile error"));
    assert_eq!(ctx.route_attempt_count(), 6);
    assert_eq!(rejected.requests().len(), 6);
    assert!(next.requests().is_empty());
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn exhausted_model_pricing_candidates_preserve_original_error_without_retry() {
    let rejected = Upstream::start(StatusCode::BAD_REQUEST, false, PRICING_BODY).await;
    let fixture = TestState::new();
    let mut ctx = context(false, vec![candidate(&rejected, "only-pricing")]);
    let error = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("model_price_error"));
    assert_eq!(rejected.requests().len(), 1);
    assert_eq!(ctx.route_attempt_count(), 1);
    rejected.finish().await;
    fixture.finish().await;
}
