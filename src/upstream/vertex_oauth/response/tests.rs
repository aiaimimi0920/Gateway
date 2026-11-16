//! Full Vertex exercises shared algorithms with its own label/source and never borrows Express credentials.
use super::*;
use serde_json::json;
fn event(raw: Value) -> Vec<u8> {
    format!("data: {raw}\n\n").into_bytes()
}
fn candidate(reason: &str) -> Value {
    json!({"candidates":[{"index":0,"content":{"parts":[{"text":"answer"}]},"finishReason":reason}]})
}
fn parse(bytes: &[u8]) -> Result<CanonicalRelayResponse, GatewayError> {
    project(CONTRACT.parse(bytes)?, "fallback-model")
}
#[test]
fn vertex_oauth_response_needs_real_finished_candidate_and_complete_frames() {
    for raw in [
        Vec::new(),
        b": ping\n\n".to_vec(),
        b"data: [DONE]\n\n".to_vec(),
        event(candidate("")),
        event(candidate("FINISH_REASON_UNSPECIFIED")),
        event(candidate("UNKNOWN")),
        event(json!({"candidates":[],"usageMetadata":{"totalTokenCount":5}})),
        event(json!({"candidates":[{"content":{"parts":[{"text":"private-partial"}]}}]})),
        b"data: {}\n".to_vec(),
        b"data: \xff\n\n".to_vec(),
    ] {
        let error = parse(&raw).unwrap_err();
        assert_eq!(error.code.as_deref(), Some("vertex_oauth_invalid_response"));
        assert_eq!(error.provider_name.as_deref(), Some("vertex_user_oauth"));
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
        assert!(!error.message.contains("private-partial") && !error.message.contains("Express"));
    }
}
#[test]
fn vertex_oauth_response_rejects_damage_or_candidate_data_after_finish() {
    for tail in [
        event(candidate("STOP")),
        b"data: {broken}\n\n".to_vec(),
        b"data: {}\n".to_vec(),
        event(json!({"error":{"message":"private-diagnostic"}})),
        b"event: error\ndata: {}\n\n".to_vec(),
        event(json!({"candidates":[{"content":{"parts":[{"text":"late"}]}}]})),
    ] {
        let error = parse(&[event(candidate("STOP")), tail].concat()).unwrap_err();
        assert!(!error.message.contains("private-diagnostic"));
        assert!(!error.retryable);
    }
}
#[test]
fn vertex_oauth_response_never_merges_choices_or_unsupported_parts_into_success() {
    for raw in [
        json!({"candidates":[{"index":0,"finishReason":"STOP"},{"index":1,"finishReason":"STOP"}]}),
        json!({"candidates":[{"index":1,"finishReason":"STOP"}]}),
        json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"tool","args":[]}}]},"finishReason":"STOP"}]}),
        json!({"candidates":[{"content":{"parts":[{"text":"answer","thoughtSignature":"opaque"}]},"finishReason":"STOP"}]}),
        json!({"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"image/png","data":"opaque"}}]},"finishReason":"STOP"}]}),
    ] {
        assert!(parse(&event(raw)).is_err());
    }
}
#[test]
fn vertex_oauth_response_omits_thought_text_and_withholds_filtered_content() {
    let raw = json!({"modelVersion":"upstream-model","candidates":[{"content":{"parts":[{"thought":true,"text":"private-thought"},{"text":"A"}]}}]});
    let response = parse(&[event(raw.clone()), event(candidate("MAX_TOKENS"))].concat()).unwrap();
    assert_eq!(response.text, "Aanswer");
    assert_eq!(response.model, "upstream-model");
    assert_eq!(response.finish_reason.as_deref(), Some("length"));
    for reason in [
        "SAFETY",
        "RECITATION",
        "BLOCKLIST",
        "PROHIBITED_CONTENT",
        "SPII",
        "MODEL_ARMOR",
        "IMAGE_SAFETY",
        "IMAGE_PROHIBITED_CONTENT",
        "IMAGE_RECITATION",
    ] {
        let response = parse(&[event(raw.clone()), event(candidate(reason))].concat()).unwrap();
        assert!(response.text.is_empty() && response.tool_calls.is_empty());
        assert_eq!(response.finish_reason.as_deref(), Some("content_filter"));
    }
    for reason in [
        "OTHER",
        "MALFORMED_FUNCTION_CALL",
        "UNEXPECTED_TOOL_CALL",
        "NO_IMAGE",
        "IMAGE_OTHER",
    ] {
        assert!(parse(&event(candidate(reason))).is_err());
    }
    let error = parse(&event(
        json!({"promptFeedback":{"blockReason":"SAFETY","blockReasonMessage":"private"}}),
    ))
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("vertex_oauth_prompt_blocked"));
    assert!(!error.message.contains("private"));
}
#[test]
fn vertex_oauth_response_preserves_total_zero_and_cache_without_inventing_missing_usage() {
    let mut raw = candidate("STOP");
    for total in [0, 17] {
        raw["usageMetadata"] = json!({"promptTokenCount":9,"candidatesTokenCount":3,"totalTokenCount":total,"cachedContentTokenCount":2});
        let parsed = parse(&event(raw.clone())).unwrap();
        let usage = parsed.usage.unwrap();
        assert_eq!(
            (
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens,
                usage.cache_read_input_tokens
            ),
            (9, 3, total, Some(2))
        );
    }
    for metadata in [json!({}), json!({"promptTokenCount":9})] {
        raw["usageMetadata"] = metadata;
        assert!(parse(&event(raw.clone())).unwrap().usage.is_none());
    }
    let parsed = parse(&event(candidate("STOP"))).unwrap();
    assert!(parsed.usage.is_none());
    assert_eq!(parsed.model, "fallback-model");
    let with_tail = parse(
        &[
            event(candidate("STOP")),
            event(json!({"usageMetadata":{"totalTokenCount":17}})),
        ]
        .concat(),
    )
    .unwrap();
    assert_eq!(with_tail.usage.unwrap().total_tokens, 17);
}
#[test]
fn vertex_oauth_response_rejects_unrepresentable_token_counters() {
    for name in [
        "promptTokenCount",
        "candidatesTokenCount",
        "totalTokenCount",
        "cachedContentTokenCount",
    ] {
        for bad in [json!(-1), json!(1.5), json!("5"), json!(u64::MAX)] {
            let mut raw = candidate("STOP");
            raw["usageMetadata"] = json!({"totalTokenCount":5});
            raw["usageMetadata"][name] = bad;
            assert!(parse(&event(raw)).is_err(), "{name}");
        }
    }
}
#[tokio::test]
async fn vertex_oauth_response_requires_sse_mime_before_success() {
    let response = rquest::Response::from(
        axum::http::Response::builder()
            .header("content-type", "application/json")
            .body(event(candidate("STOP")))
            .unwrap(),
    );
    let error = accumulate(async { Ok(response) }, "model")
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("vertex_oauth_invalid_response"));
    assert!(!error.retryable);
}
