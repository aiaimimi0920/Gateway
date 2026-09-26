use std::time::Duration;

use futures::StreamExt;

use super::body_targets::{client, read_json, Target, CHAT_PATH, RESPONSES_PATH};
use super::body_wire::{BodyMode, WireServer, FIRST_SSE};
use super::{make_payload, make_request, UpstreamStreamingResponse};
use crate::error::{ErrorKind, FallbackHint};
use crate::protocol::canonical::EndpointKind;
use crate::protocol::chatgpt::official_api as surface;

async fn error_charset(target: Target) {
    let server = WireServer::start(
        target.path(),
        429,
        "application/json; charset=windows-1252",
        BodyMode::Finite(
            b"{\"error\":{\"message\":\"caf\xe9\",\"code\":\"fixture_quota\"},\"retry_after\":2}",
        ),
    )
    .await;
    let result = tokio::time::timeout(Duration::from_secs(8), target.invoke(&server.url)).await;
    server.stop().await;
    let error = result
        .expect("finite error body")
        .expect_err("upstream failure");
    assert_eq!(error.kind, ErrorKind::RateLimit);
    assert_eq!(error.http_status, Some(429));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(surface::CHATGPT_OFFICIAL_API_PROFILE)
    );
    assert_eq!(error.message, "caf\u{e9}");
    assert_eq!(error.code.as_deref(), Some("fixture_quota"));
    assert!(matches!(
        error.fallback_hint,
        FallbackHint::Retry { delay_ms: 2000, .. }
    ));
}

async fn unreadable_error(target: Target) {
    let server =
        WireServer::start(target.path(), 503, "application/json", BodyMode::Truncated).await;
    let result = tokio::time::timeout(Duration::from_secs(8), target.invoke(&server.url)).await;
    server.stop().await;
    let error = result
        .expect("truncated error body")
        .expect_err("upstream failure");
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(surface::CHATGPT_OFFICIAL_API_PROFILE)
    );
    assert_eq!(error.message, "<unreadable body>");
    assert_eq!(error.code, None);
}

#[tokio::test]
async fn nonstream_error_preserves_charset_and_retry() {
    error_charset(Target::NonstreamError).await;
}
#[tokio::test]
async fn forced_error_preserves_charset_and_retry() {
    error_charset(Target::ForcedError).await;
}
#[tokio::test]
async fn stream_error_preserves_charset_and_retry() {
    error_charset(Target::StreamError).await;
}
#[tokio::test]
async fn nonstream_error_preserves_unreadable_fallback() {
    unreadable_error(Target::NonstreamError).await;
}
#[tokio::test]
async fn forced_error_preserves_unreadable_fallback() {
    unreadable_error(Target::ForcedError).await;
}
#[tokio::test]
async fn stream_error_preserves_unreadable_fallback() {
    unreadable_error(Target::StreamError).await;
}

#[tokio::test]
async fn json_accepts_exact_accumulation_limit() {
    let server = WireServer::start(CHAT_PATH, 200, "application/json", BodyMode::ExactLimit).await;
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        read_json(&server.url, EndpointKind::ChatCompletions),
    )
    .await;
    server.stop().await;
    let response = result
        .expect("finite exact-limit JSON")
        .expect("canonical response");
    assert_eq!(response.text, "Paris");
    assert_eq!(response.model, "fixture-model");
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn json_keeps_utf8_even_when_http_charset_disagrees() {
    let body =
        b"{\"model\":\"fixture-model\",\"choices\":[{\"message\":{\"content\":\"caf\xc3\xa9\"}}]}";
    let server = WireServer::start(
        CHAT_PATH,
        200,
        "application/json; charset=windows-1252",
        BodyMode::Finite(body),
    )
    .await;
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        read_json(&server.url, EndpointKind::ChatCompletions),
    )
    .await;
    server.stop().await;
    assert_eq!(
        result.expect("finite JSON").expect("UTF-8 JSON").text,
        "caf\u{e9}"
    );
}

#[tokio::test]
async fn json_rejects_invalid_utf8_and_malformed_json_with_provider() {
    for body in [
        b"{\"choices\":[{\"message\":{\"content\":\"caf\xe9\"}}]}".as_slice(),
        b"{",
    ] {
        let server = WireServer::start(
            CHAT_PATH,
            200,
            "application/json; charset=windows-1252",
            BodyMode::Finite(body),
        )
        .await;
        let result = tokio::time::timeout(
            Duration::from_secs(8),
            read_json(&server.url, EndpointKind::ChatCompletions),
        )
        .await;
        server.stop().await;
        let error = result
            .expect("finite invalid JSON")
            .expect_err("invalid JSON");
        assert_eq!(error.kind, ErrorKind::Unknown);
        assert_eq!(
            error.provider_name.as_deref(),
            Some(surface::CHATGPT_OFFICIAL_API_PROFILE)
        );
        assert_eq!(error.http_status, None);
        assert_eq!(error.code, None);
        assert!(error.message.starts_with("Network error:"));
    }
}

#[tokio::test]
async fn truncated_json_retains_transport_error_classification() {
    let server = WireServer::start(CHAT_PATH, 200, "application/json", BodyMode::Truncated).await;
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        read_json(&server.url, EndpointKind::ChatCompletions),
    )
    .await;
    server.stop().await;
    let error = result
        .expect("truncated JSON")
        .expect_err("transport error");
    assert_eq!(error.kind, ErrorKind::Unknown);
    assert_eq!(
        error.provider_name.as_deref(),
        Some(surface::CHATGPT_OFFICIAL_API_PROFILE)
    );
    assert_eq!(error.code, None);
}

#[tokio::test]
async fn responses_json_preserves_usage_and_tool_calls() {
    let body = br#"{"id":"resp-fixture","model":"fixture-model","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"Paris"}]},{"type":"function_call","call_id":"call-fixture","name":"lookup_city","arguments":"{\"city\":\"Paris\"}"}],"usage":{"input_tokens":4,"output_tokens":6,"total_tokens":10}}"#;
    let server = WireServer::start(
        RESPONSES_PATH,
        200,
        "application/json",
        BodyMode::Finite(body),
    )
    .await;
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        read_json(&server.url, EndpointKind::Responses),
    )
    .await;
    server.stop().await;
    let response = result
        .expect("finite Responses JSON")
        .expect("canonical response");
    assert_eq!(response.text, "Paris");
    assert_eq!(response.model, "fixture-model");
    let usage = response.usage.expect("usage");
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (4, 6, 10)
    );
    let call = &response.tool_calls[0];
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(call.id.as_deref(), Some("call-fixture"));
    assert_eq!(call.name.as_deref(), Some("lookup_city"));
    assert_eq!(call.arguments.as_deref(), Some(r#"{"city":"Paris"}"#));
}

#[tokio::test]
async fn live_stream_preserves_headers_and_output_before_completion() {
    let mut server =
        WireServer::start(CHAT_PATH, 201, "text/event-stream", BodyMode::HeldSse).await;
    let client = client();
    let mut request = make_request(EndpointKind::ChatCompletions);
    request.stream = true;
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        client.execute_chatgpt_official_streaming(
            &make_payload(&format!("{}/api.openai.com", server.url)),
            &request,
            "fixture-model",
            None,
        ),
    )
    .await
    .expect("stream returns while upstream held open")
    .expect("stream");
    let UpstreamStreamingResponse::Http(response) = result else {
        panic!("raw HTTP response");
    };
    assert_eq!(response.status().as_u16(), 201);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-fixture-origin"], "official-loopback");
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while body.len() < FIRST_SSE.len() {
            body.extend_from_slice(&stream.next().await.expect("first SSE").expect("body chunk"));
            assert!(body.len() <= 4096);
        }
    })
    .await
    .expect("first output before upstream completion");
    assert_eq!(body, FIRST_SSE);
    server.complete();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(chunk) = stream.next().await {
            body.extend_from_slice(&chunk.expect("remaining body"));
            assert!(body.len() <= 4096);
        }
    })
    .await
    .expect("stream completion");
    server.stop().await;
    assert_eq!(body, [FIRST_SSE, b"data: [DONE]\n\n"].concat());
}
