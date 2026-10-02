//! 通过真实 loopback HTTP 和 stage_send 验证接线，不访问真实凭证、数据库或付费上游。
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;

use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::pipeline::{stage_send, PipelineOutput};
use neuro_gateway::protocol::canonical::{EndpointKind, ProtocolFamily};
use std::sync::atomic::Ordering;
use tokio::time::timeout;

use fixture::{candidate, context, TestState, Upstream, DEADLINE, SSE_REPLY};

async fn translated_endpoint(
    endpoint: EndpointKind,
    expected: &str,
    input_usage: &str,
    output_usage: &str,
    gemini: bool,
) {
    let upstream = Upstream::start(StatusCode::OK, true, SSE_REPLY).await;
    let fixture = TestState::new();
    let mut selected = candidate(&upstream, "typed-translated");
    // 明确声明仅支持 chat 上游，Responses 请求据此进入生产桥接路径。
    selected.payload.chat_completions_path = Some("/v1/chat/completions".to_string());
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = context(true, vec![selected]);
    ctx.canonical_req.endpoint_kind = endpoint;
    if endpoint == EndpointKind::Messages {
        ctx.canonical_req.protocol_family = ProtocolFamily::Anthropic;
    } else if gemini {
        ctx.canonical_req.protocol_family = ProtocolFamily::GeminiGenerateContent;
    }
    let output = timeout(DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(stream) = output else {
        panic!("expected typed SSE");
    };
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    let mut stream: std::pin::Pin<
        Box<
            dyn futures::Stream<
                    Item = Result<
                        bytes::Bytes,
                        neuro_gateway::protocol::stream_error::StreamError<rquest::Error>,
                    >,
                > + Send,
        >,
    > = if gemini {
        Box::pin(
            neuro_gateway::protocol::gemini_api::translate_openai_sse_to_gemini_stream_with_error(
                stream,
                "gemini-test".into(),
            ),
        )
    } else {
        Box::pin(stream)
    };
    let bytes = timeout(DEADLINE, async {
        let mut output = Vec::new();
        while let Some(chunk) = stream.next().await {
            output.extend_from_slice(&chunk.unwrap());
            assert!(output.len() <= 64 * 1024);
        }
        output
    })
    .await
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains(expected), "{text}");
    assert!(text.contains("local stream reply"));
    assert!(text.contains(input_usage), "{text}");
    assert!(text.contains(output_usage), "{text}");
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    drop(stream);
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 1);
    let requests = upstream.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert!(requests[0].fixture_auth);
    assert_eq!(requests[0].body["stream"], true);
    upstream.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn responses_stream_is_wired_through_typed_translation_and_observation() {
    translated_endpoint(
        EndpointKind::Responses,
        "response.completed",
        "\"input_tokens\":7",
        "\"output_tokens\":3",
        false,
    )
    .await;
}

#[tokio::test]
async fn messages_stream_is_wired_through_typed_translation_and_observation() {
    translated_endpoint(
        EndpointKind::Messages,
        "message_stop",
        "\"input_tokens\":7",
        "\"output_tokens\":3",
        false,
    )
    .await;
}

#[tokio::test]
async fn completions_stream_is_wired_through_typed_translation_and_observation() {
    translated_endpoint(
        EndpointKind::Completions,
        "[DONE]",
        "\"prompt_tokens\":7",
        "\"completion_tokens\":3",
        false,
    )
    .await;
}

#[tokio::test]
async fn gemini_stream_preserves_real_http_usage_and_exactly_once_tracking() {
    translated_endpoint(
        EndpointKind::ChatCompletions,
        "\"finishReason\":\"STOP\"",
        "\"promptTokenCount\":7",
        "\"candidatesTokenCount\":3",
        true,
    )
    .await;
}
