use super::*;
mod preview;
mod signed;
mod tools;
fn event(raw: Value) -> Vec<u8> {
    format!("data: {raw}\n\n").into_bytes()
}
fn candidate(reason: &str) -> Value {
    json!({"candidates":[{"content":{"parts":[{"text":"OK"}]},"finishReason":reason}]})
}
#[test]
fn vertex_express_completion_requires_finished_candidate_not_eof_or_openai_done() {
    for raw in [
        Vec::new(),
        b": keepalive\n\n".to_vec(),
        b"data: [DONE]\n\n".to_vec(),
        event(json!({"candidates":[{"content":{"parts":[{"text":"partial"}]}}]})),
        event(candidate("FINISH_REASON_UNSPECIFIED")),
        event(candidate("")),
        event(candidate("unverified-finish")),
        event(json!({"candidates":[],"usageMetadata":{"totalTokenCount":5}})),
    ] {
        let error = parse(&raw, "model").unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("vertex_express_invalid_response")
        );
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
}
#[test]
fn vertex_express_completion_rejects_late_errors_damage_and_unflushed_frames() {
    let good = event(candidate("STOP"));
    for tail in [
        b"data: {broken}\n\n".to_vec(),
        b"data: \xff\n\n".to_vec(),
        b"data: {\"unfinished\":".to_vec(),
        b"data: {}\n".to_vec(),
        event(json!({"error":{"message":"secret-upstream-detail"}})),
        event(candidate("STOP")),
        b"event: error\ndata: {}\n\n".to_vec(),
    ] {
        let error = parse(&[good.clone(), tail].concat(), "model").unwrap_err();
        assert!(!error.message.contains("secret-upstream-detail"));
        assert!(!error.retryable);
    }
}
#[test]
fn vertex_express_completion_projects_fragmented_parts_finish_usage_and_cache_without_guessing() {
    let body = [event(json!({"modelVersion":"version","candidates":[{"content":{"parts":[{"thought":true,"text":"hidden"},{"text":"A"}]}}]})),
        event(json!({"candidates":[{"content":{"parts":[{"text":"B"}]},"finishReason":"MAX_TOKENS"}]})),
        event(json!({"usageMetadata":{"promptTokenCount":9,"candidatesTokenCount":3,"totalTokenCount":17,"cachedContentTokenCount":2,"thoughtsTokenCount":5}}))].concat();
    let parsed = parse(&body, "model").unwrap();
    assert_eq!(parsed.text, "AB");
    assert_eq!(parsed.model, "version");
    assert_eq!(parsed.finish_reason.as_deref(), Some("length"));
    let usage = parsed.usage.unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
            usage.cache_read_input_tokens
        ),
        (9, 3, 17, Some(2))
    );
    assert_eq!(usage.cache_creation_input_tokens, None);
    let parsed = parse(&event(candidate("STOP")), "model").unwrap();
    assert!(parsed.usage.is_none());
    for metadata in [json!({}), json!({"promptTokenCount":9})] {
        let mut raw = candidate("STOP");
        raw["usageMetadata"] = metadata;
        assert!(parse(&event(raw), "model").unwrap().usage.is_none());
    }
}
#[test]
fn vertex_express_completion_keeps_explicit_zero_usage_and_rejects_invalid_counters() {
    let mut raw = candidate("STOP");
    raw["usageMetadata"] =
        json!({"promptTokenCount":0,"candidatesTokenCount":0,"totalTokenCount":0});
    assert_eq!(
        parse(&event(raw.clone()), "model")
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        0
    );
    for name in [
        "promptTokenCount",
        "candidatesTokenCount",
        "totalTokenCount",
        "cachedContentTokenCount",
    ] {
        for bad in [json!(-1), json!(1.5), json!("5"), json!(u64::MAX)] {
            raw["usageMetadata"][name] = bad;
            assert!(parse(&event(raw.clone()), "model").is_err());
        }
        raw["usageMetadata"][name] = json!(0);
    }
}
#[test]
fn vertex_express_completion_never_merges_multiple_choices_into_one_answer() {
    for raw in [
        json!({"candidates":[{"index":0,"finishReason":"STOP"},{"index":1,"finishReason":"STOP"}]}),
        json!({"candidates":[{"index":1,"finishReason":"STOP"}]}),
    ] {
        assert!(parse(&event(raw), "model").is_err());
    }
    let blocked =
        event(json!({"promptFeedback":{"blockReason":"SAFETY","blockReasonMessage":"private"}}));
    let error = parse(&blocked, "model").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("vertex_express_prompt_blocked"));
    assert!(!error.message.contains("private"));
    let filtered = parse(
        &event(json!({"candidates":[{"finishReason":"SAFETY"}]})),
        "model",
    )
    .unwrap();
    assert_eq!(filtered.finish_reason.as_deref(), Some("content_filter"));
}
