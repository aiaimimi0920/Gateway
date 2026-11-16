//! Complete function parts must preserve call identity; filters never authorize execution.
use super::*;

fn response(parts: Value, reason: &str) -> Vec<u8> {
    event(
        json!({"candidates":[{"content":{"parts":parts},"finishReason":reason}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}}),
    )
}

#[test]
fn vertex_express_tools_preserve_identical_calls_and_allocate_unique_roundtrip_ids() {
    let part =
        json!({"functionCall":{"name":"lookup","args":{"城市":"杭州","nested":[1,true,null]}}});
    let body = response(
        json!([part,part,{"functionCall":{"id":"call_vertex_express_0","name":"noop","args":{}}}]),
        "STOP",
    );
    let parsed = parse(&body, "model").unwrap();
    assert_eq!(parsed.tool_calls.len(), 3);
    let ids: std::collections::HashSet<_> = parsed
        .tool_calls
        .iter()
        .map(|c| c.id.as_deref().unwrap())
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.iter().all(|id| !id.is_empty()));
    assert_eq!(
        parsed.tool_calls[2].id.as_deref(),
        Some("call_vertex_express_0")
    );
    assert_eq!(
        parsed.tool_calls[0].arguments,
        parsed.tool_calls[1].arguments
    );
    assert_eq!(
        serde_json::from_str::<Value>(parsed.tool_calls[0].arguments.as_deref().unwrap()).unwrap(),
        part["functionCall"]["args"]
    );
    assert_eq!(parsed.finish_reason.as_deref(), Some("tool_calls"));
    // Separate turns must not reuse fabricated identities, even for identical calls/arguments.
    assert_ne!(
        parse(&body, "model").unwrap().tool_calls[0].id,
        parsed.tool_calls[0].id
    );
}

#[test]
fn vertex_express_tools_keep_explicit_identity_empty_arguments_and_length_finish() {
    let parsed = parse(
        &response(
            json!([{"text":"before"},{"functionCall":{"id":"explicit","name":"noop","args":{}}}]),
            "MAX_TOKENS",
        ),
        "model",
    )
    .unwrap();
    assert_eq!(parsed.text, "before");
    assert_eq!(parsed.tool_calls[0].id.as_deref(), Some("explicit"));
    assert_eq!(parsed.tool_calls[0].arguments.as_deref(), Some("{}"));
    assert_eq!(parsed.finish_reason.as_deref(), Some("length"));
    let omitted = parse(
        &response(json!([{"functionCall":{"name":"noop"}}]), "STOP"),
        "model",
    )
    .unwrap();
    assert_eq!(omitted.tool_calls[0].arguments.as_deref(), Some("{}"));
}

#[test]
fn vertex_express_tools_reject_invalid_or_fragmented_calls_without_empty_argument_repair() {
    for call in [
        json!(null),
        json!({}),
        json!({"name":1,"args":{}}),
        json!({"name":" ","args":{}}),
        json!({"name":"lookup","args":"{broken"}),
        json!({"name":"lookup","args":[]}),
        json!({"name":"lookup","args":null}),
        json!({"name":"lookup","args":{},"id":1}),
        json!({"name":"lookup","args":{},"id":""}),
        json!({"name":"lookup","partialArgs":[{"jsonPath":"$.city","stringValue":"杭"}],"willContinue":true}),
        json!({"name":"lookup","args":{},"willContinue":false}),
    ] {
        let error = parse(&response(json!([{"functionCall":call}]), "STOP"), "model").unwrap_err();
        assert_eq!(error.http_status, Some(502));
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
}

#[test]
fn vertex_express_tools_reject_duplicate_or_conflicting_explicit_identity() {
    for args in [json!({}), json!({"different":true})] {
        assert!(parse(
            &response(
                json!([
                    {"functionCall":{"id":"same","name":"lookup","args":{}}},
                    {"functionCall":{"id":"same","name":"lookup","args":args}}
                ]),
                "STOP"
            ),
            "model"
        )
        .is_err());
    }
}

#[test]
fn vertex_express_tools_filters_discard_prior_text_and_executable_calls_but_keep_usage() {
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
        let first = event(
            json!({"candidates":[{"content":{"parts":[{"text":"must-not-escape"},{"functionCall":{"name":"dangerous","args":{}}}]}}]}),
        );
        let parsed = parse(&[first, response(json!([]), reason)].concat(), "model").unwrap();
        assert!(parsed.text.is_empty(), "{reason}");
        assert!(parsed.tool_calls.is_empty(), "{reason}");
        assert_eq!(
            parsed.finish_reason.as_deref(),
            Some("content_filter"),
            "{reason}"
        );
        assert_eq!(parsed.usage.unwrap().total_tokens, 5);
    }
}

#[test]
fn vertex_express_tools_generation_failures_are_not_normal_success_or_replayed() {
    for reason in [
        "MALFORMED_FUNCTION_CALL",
        "UNEXPECTED_TOOL_CALL",
        "OTHER",
        "IMAGE_OTHER",
        "NO_IMAGE",
    ] {
        let error =
            parse(&response(json!([{"text":"not-complete"}]), reason), "model").unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("vertex_express_invalid_response")
        );
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
}
