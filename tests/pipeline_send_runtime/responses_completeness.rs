//! Real loopback wire selection, terminal accounting and post-handoff no-retry contract.
use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use neuro_gateway::protocol::canonical::{EndpointKind, ProtocolFamily};
use neuro_gateway::protocol::stream_error::StreamError;
use std::sync::atomic::Ordering;
use tokio::time::timeout;

use super::fixture::{
    authorize_candidates, candidate, context, TestState, Upstream, DEADLINE, SSE_REPLY,
};

const PARTIAL: &str = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n";
const COMPLETE: &str = "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":2,\"output_tokens\":3,\"total_tokens\":5}}}\n\n";

#[tokio::test]
async fn truncated_responses_never_reports_success_or_retries_after_handoff() {
    for endpoint in [
        EndpointKind::Responses,
        EndpointKind::ChatCompletions,
        EndpointKind::Messages,
    ] {
        let upstream = Upstream::start(StatusCode::OK, true, PARTIAL).await;
        let fallback = Upstream::start(StatusCode::OK, true, COMPLETE).await;
        let fixture = TestState::new();
        let mut selected = candidate(&upstream, "responses-truncated");
        selected.payload.responses_path = Some("/v1/responses".into());
        selected.payload.chat_completions_path = None;
        let controller = fixture
            .state
            .concurrency_registry
            .get_or_create(&selected.provider_account_id);
        let mut second = candidate(&fallback, "responses-fallback");
        second.payload.responses_path = Some("/v1/responses".into());
        second.payload.chat_completions_path = None;
        let mut ctx = context(true, vec![selected, second]);
        ctx.canonical_req.endpoint_kind = endpoint;
        if endpoint == EndpointKind::Messages {
            ctx.canonical_req.protocol_family = ProtocolFamily::Anthropic;
        }
        authorize_candidates(&mut ctx);
        let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap();
        let PipelineOutput::Sse(stream) = output else {
            panic!("expected SSE")
        };
        let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
        let errors: Vec<_> = chunks
            .iter()
            .filter_map(|item| item.as_ref().err())
            .collect();
        assert_eq!(errors.len(), 1, "endpoint {endpoint:?}");
        let output = chunks
            .iter()
            .filter_map(|item| item.as_ref().ok())
            .flat_map(|bytes| bytes.iter().copied())
            .collect::<Vec<_>>();
        let output = String::from_utf8(output).unwrap();
        for successful_terminal in ["[DONE]", "message_stop", "\"finish_reason\":\"stop\""] {
            assert!(
                !output.contains(successful_terminal),
                "endpoint {endpoint:?}: {output}"
            );
        }
        assert!(matches!(errors[0], StreamError::Protocol(_)));
        assert!(errors[0]
            .to_string()
            .contains("responses_stream_incomplete"));
        assert_eq!(controller.snapshot().consecutive_successes, 0);
        assert_eq!(controller.snapshot().active_count, 0);
        assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 1);
        assert!(
            fallback.requests().is_empty(),
            "SSE handoff cannot cause a second paid request"
        );
        assert_eq!(upstream.requests()[0].path, "/v1/responses");
        upstream.finish().await;
        fallback.finish().await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn native_responses_preserves_wire_and_records_one_success() {
    let upstream = Upstream::start(StatusCode::OK, true, COMPLETE).await;
    let fixture = TestState::new();
    let mut selected = candidate(&upstream, "responses-complete");
    selected.payload.responses_path = Some("/v1/responses".into());
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = context(true, vec![selected]);
    ctx.canonical_req.endpoint_kind = EndpointKind::Responses;
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = output else {
        panic!("expected SSE")
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    let bytes: Vec<_> = chunks
        .into_iter()
        .flat_map(|item| item.unwrap().to_vec())
        .collect();
    assert_eq!(bytes, COMPLETE.as_bytes());
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    assert_eq!(upstream.requests()[0].path, "/v1/responses");
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn responses_request_bridged_to_chat_does_not_require_responses_terminal() {
    let upstream = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let mut selected = candidate(&upstream, "responses-chat-bridge");
    selected.payload.chat_completions_path = Some("/v1/chat/completions".into());
    selected.payload.responses_path = None;
    let mut ctx = context(true, vec![selected]);
    ctx.canonical_req.endpoint_kind = EndpointKind::Responses;
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = output else {
        panic!("expected SSE")
    };
    let chunks = timeout(DEADLINE, stream.collect::<Vec<_>>()).await.unwrap();
    assert!(chunks.iter().all(Result::is_ok));
    let bytes: Vec<_> = chunks
        .into_iter()
        .flat_map(|item| item.unwrap().to_vec())
        .collect();
    assert!(String::from_utf8_lossy(&bytes).contains("response.completed"));
    assert_eq!(upstream.requests()[0].path, "/v1/chat/completions");
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn buffered_responses_requires_completion_on_real_forced_streaming_wire() {
    for (wire, complete) in [(PARTIAL, false), (COMPLETE, true)] {
        let upstream = Upstream::start(StatusCode::OK, true, wire).await;
        let fixture = TestState::new();
        let mut selected = candidate(&upstream, "responses-buffered");
        selected.payload.responses_path = Some("/v1/responses".into());
        selected.payload.chat_completions_path = None;
        let controller = fixture
            .state
            .concurrency_registry
            .get_or_create(&selected.provider_account_id);
        let mut ctx = context(false, vec![selected]);
        ctx.canonical_req.endpoint_kind = EndpointKind::Responses;
        let result = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap();
        if complete {
            let PipelineOutput::Json(reply) = result.unwrap() else {
                panic!("expected buffered JSON")
            };
            assert_eq!(reply["status"], "completed");
            assert_eq!(reply["usage"]["total_tokens"], 5);
            assert_eq!(controller.snapshot().consecutive_successes, 1);
        } else {
            let error = result
                .err()
                .expect("truncated stream must not produce JSON success");
            assert_eq!(error.code.as_deref(), Some("responses_stream_incomplete"));
            assert_eq!(controller.snapshot().consecutive_successes, 0);
        }
        assert_eq!(controller.snapshot().active_count, 0);
        assert!(upstream
            .requests()
            .iter()
            .all(|request| request.path == "/v1/responses" && request.body["stream"] == true));
        upstream.finish().await;
        fixture.finish().await;
    }
}
