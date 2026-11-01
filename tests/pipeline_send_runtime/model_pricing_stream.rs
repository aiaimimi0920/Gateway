//! Early SSE pricing refusals can advance; already emitted content must never replay.
use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, SSE_REPLY,
};
use super::model_pricing::{with_tools, RESPONSES_REPLY};
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use neuro_gateway::protocol::canonical::EndpointKind;
use tokio::time::timeout;

const ERROR_EVENT: &str = "event: error\ndata: {\"error\":{\"code\":\"model_price_error\",\"message\":\"model gpt-6.1-sol tiered expr run failed: expr compile error: unknown name cc12h (1:62)\"}}\n\n";
const FAILED_RESPONSE: &str = "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"model_price_error\",\"message\":\"model gpt-6.1-sol tiered expr run failed: expr compile error: unknown name cc12h (1:62)\"}}}\n\n";

#[tokio::test]
async fn model_pricing_early_sse_errors_fall_back_before_tools_or_output() {
    for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Responses] {
        let responses = endpoint == EndpointKind::Responses;
        let rejected = Upstream::start(
            StatusCode::OK,
            true,
            if responses {
                FAILED_RESPONSE
            } else {
                ERROR_EVENT
            },
        )
        .await;
        let next = Upstream::start(
            StatusCode::OK,
            true,
            if responses {
                RESPONSES_REPLY
            } else {
                SSE_REPLY
            },
        )
        .await;
        let fixture = TestState::new();
        let mut ctx = context(
            true,
            vec![
                candidate(&rejected, "pricing-sse"),
                candidate(&next, "healthy"),
            ],
        );
        ctx.canonical_req.endpoint_kind = endpoint;
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
            .unwrap_or_else(|e| panic!("early SSE pricing refusal must advance: {e:?}"));
        let PipelineOutput::Sse(stream) = output else {
            panic!("expected SSE");
        };
        let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
        assert!(chunks.iter().all(Result::is_ok));
        let wire = String::from_utf8(
            chunks
                .iter()
                .filter_map(|c| c.as_ref().ok())
                .flat_map(|c| c.iter().copied())
                .collect(),
        )
        .unwrap();
        assert!(!wire.contains("model_price_error"));
        assert_eq!((rejected.requests().len(), next.requests().len()), (1, 1));
        assert_eq!(ctx.route_attempt_count(), 2);
        rejected.finish().await;
        next.finish().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn model_pricing_error_after_output_does_not_replay() {
    let body = concat!(
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n",
        "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"model_price_error\",\"message\":\"model gpt-6.1-sol tiered expr run failed: expr compile error: unknown name cc12h (1:62)\"}}}\n\n",
    );
    let rejected = Upstream::start(StatusCode::OK, true, body).await;
    let next = Upstream::start(StatusCode::OK, true, RESPONSES_REPLY).await;
    let fixture = TestState::new();
    let mut ctx = context(
        true,
        vec![
            candidate(&rejected, "partial"),
            candidate(&next, "untouched"),
        ],
    );
    ctx.canonical_req.endpoint_kind = EndpointKind::Responses;
    for c in &mut ctx.candidates {
        c.payload.responses_path = Some("/v1/responses".into());
        c.payload.chat_completions_path = None;
    }
    with_tools(&mut ctx);
    authorize_candidates(&mut ctx);
    let PipelineOutput::Sse(stream) = stage_send::run(&mut ctx, &fixture.state).await.unwrap()
    else {
        panic!("expected SSE");
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    assert!(chunks.iter().any(Result::is_err));
    assert!(next.requests().is_empty());
    assert_eq!(ctx.route_attempt_count(), 1);
    rejected.finish().await;
    next.finish().await;
    fixture.finish().await;
}
