//! Official Preview fragments form complete calls, never one executable call per chunk.
use super::*;

fn stream(calls: Vec<Value>, reason: Option<&str>) -> Vec<u8> {
    let mut body: Vec<u8> = calls
        .into_iter()
        .flat_map(|call| {
            event(json!({"candidates":[{"content":{"parts":[{"functionCall":call}]}}]}))
        })
        .collect();
    if let Some(reason) = reason {
        body.extend(event(
            json!({"candidates":[{"finishReason":reason}],"usageMetadata":{"totalTokenCount":5}}),
        ));
    }
    body
}

fn arguments(calls: Vec<Value>) -> Value {
    let result = parse(&stream(calls, Some("STOP")), "model").unwrap();
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("lookup"));
    serde_json::from_str(result.tool_calls[0].arguments.as_deref().unwrap()).unwrap()
}

fn abort(body: Vec<u8>) {
    let error = parse(&body, "model").unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("vertex_express_invalid_response")
    );
    assert_eq!(error.http_status, Some(502));
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
}

#[test]
fn vertex_express_preview_official_path_only_and_empty_call_endings() {
    assert_eq!(
        arguments(vec![
            json!({"name":"lookup","partialArgs":[{"jsonPath":"$.brightness","numberValue":50}],"willContinue":true}),
            json!({"partialArgs":[{"jsonPath":"$.color","stringValue":"warm","willContinue":true}],"willContinue":true}),
            json!({"partialArgs":[{"jsonPath":"$.color"}],"willContinue":true}),
            json!({})
        ]),
        json!({"brightness":50,"color":"warm"})
    );
}

#[test]
fn vertex_express_preview_unicode_strings_nested_arrays_and_scalar_union() {
    assert_eq!(
        arguments(vec![
            json!({"name":"lookup","willContinue":true}),
            json!({"partialArgs":[{"jsonPath":"$.items[0].city","stringValue":"杭","willContinue":true},{"jsonPath":"$.enabled","boolValue":false}],"willContinue":true}),
            json!({"partialArgs":[{"jsonPath":"$.items[0].city","stringValue":"州"},{"jsonPath":"$.items[1].number","numberValue":-1.5},{"jsonPath":"$.empty","nullValue":null}]})
        ]),
        json!({"items":[{"city":"杭州"},{"number":-1.5}],"enabled":false,"empty":null})
    );
}

#[test]
fn vertex_express_preview_quoted_paths_decode_escapes_and_share_string_identity() {
    assert_eq!(
        arguments(vec![
            json!({"name":"lookup","partialArgs":[{"jsonPath":"$['a.b'][\"引号\\\"\"][0]['x\\'y']","stringValue":"A","willContinue":true}],"willContinue":true}),
            json!({"partialArgs":[{"jsonPath":"$[\"a.b\"][\"引号\\\"\"][0][\"x'y\"]","stringValue":""}],"willContinue":false})
        ]),
        json!({"a.b":{"引号\"":[{"x'y":"A"}]}})
    );
    assert_eq!(
        arguments(vec![
            json!({"name":"lookup","partialArgs":[{"jsonPath":"$['\\uD83D\\uDE00']","stringValue":"ok"}]})
        ]),
        json!({"😀":"ok"})
    );
}

#[test]
fn vertex_express_preview_sequential_identical_calls_keep_distinct_public_ids() {
    let mut calls = Vec::new();
    for _ in 0..2 {
        calls.extend([json!({"name":"lookup","willContinue":true}),json!({"partialArgs":[{"jsonPath":"$.city","stringValue":"杭州","willContinue":true}],"willContinue":true}),json!({"partialArgs":[{"jsonPath":"$.city","stringValue":""}],"willContinue":true}),json!({})]);
    }
    let parsed = parse(&stream(calls, Some("STOP")), "model").unwrap();
    assert_eq!(parsed.tool_calls.len(), 2);
    assert_ne!(parsed.tool_calls[0].id, parsed.tool_calls[1].id);
    assert_eq!(
        parsed.tool_calls[0].arguments,
        parsed.tool_calls[1].arguments
    );
    assert_eq!(
        parsed.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"杭州\"}")
    );
}

#[test]
fn vertex_express_preview_noarg_call_finishes_only_after_explicit_call_end() {
    assert_eq!(
        arguments(vec![
            json!({"name":"lookup","willContinue":true}),
            json!({})
        ]),
        json!({})
    );
    for reason in [Some("STOP"), Some("MAX_TOKENS"), None] {
        abort(stream(
            vec![json!({"name":"lookup","willContinue":true})],
            reason,
        ));
    }
}

#[test]
fn vertex_express_preview_unfinished_string_is_not_closed_by_call_or_candidate_finish() {
    for tail in [
        json!({}),
        json!({"willContinue":false}),
        json!({"partialArgs":[]}),
    ] {
        abort(stream(
            vec![
                json!({"name":"lookup","partialArgs":[{"jsonPath":"$.city","stringValue":"secret-fragment","willContinue":true}],"willContinue":true}),
                tail,
            ],
            Some("STOP"),
        ));
    }
}

#[test]
fn vertex_express_preview_rejects_malformed_union_flags_identity_and_mixed_args() {
    for fragment in [
        json!({"jsonPath":"$.x"}),
        json!({"jsonPath":"$.x","numberValue":"1"}),
        json!({"jsonPath":"$.x","boolValue":1}),
        json!({"jsonPath":"$.x","nullValue":false}),
        json!({"jsonPath":"$.x","stringValue":null}),
        json!({"jsonPath":"$.x","numberValue":1,"stringValue":"1"}),
        json!({"jsonPath":"$.x","numberValue":1,"willContinue":true}),
        json!({"jsonPath":"$.x","stringValue":"x","willContinue":"true"}),
    ] {
        abort(stream(
            vec![json!({"name":"lookup","partialArgs":[fragment]})],
            Some("STOP"),
        ));
    }
    for call in [
        json!({"name":"lookup","partialArgs":{}}),
        json!({"name":"lookup","willContinue":null}),
        json!({"name":"lookup","partialArgs":[],"args":{}}),
    ] {
        abort(stream(vec![call], Some("STOP")));
    }
    for tail in [
        json!({"name":"other"}),
        json!({"id":"other"}),
        json!({"args":{}}),
    ] {
        abort(stream(
            vec![
                json!({"id":"first","name":"lookup","willContinue":true}),
                tail,
            ],
            Some("STOP"),
        ));
    }
}

#[test]
fn vertex_express_preview_rejects_ambiguous_paths_duplicate_leaves_and_container_conflicts() {
    for path in [
        "$",
        "$.x[*]",
        "$..x",
        "$.x[-1]",
        "$.x[2]",
        "$.x[01]",
        "$.x[0,1]",
        "$.x[0:1]",
        "$.x[?@]",
        "$['bad\\q']",
        "$['\\uD800']",
        "$.0bad",
        "$[0]",
        "$.x.",
    ] {
        abort(stream(
            vec![json!({"name":"lookup","partialArgs":[{"jsonPath":path,"numberValue":1}]})],
            Some("STOP"),
        ));
    }
    for second in ["$.x", "$.x.y", "$['x']"] {
        abort(stream(
            vec![
                json!({"name":"lookup","partialArgs":[{"jsonPath":"$.x","nullValue":null},{"jsonPath":second,"stringValue":"bad"}]}),
            ],
            Some("STOP"),
        ));
    }
    abort(stream(
        vec![
            json!({"name":"lookup","partialArgs":[{"jsonPath":"$.x.y","numberValue":1},{"jsonPath":"$.x","stringValue":"bad"}]}),
        ],
        Some("STOP"),
    ));
    let deep = format!("${}", ".x".repeat(65));
    abort(stream(
        vec![json!({"name":"lookup","partialArgs":[{"jsonPath":deep,"numberValue":1}]})],
        Some("STOP"),
    ));
}

#[test]
fn vertex_express_preview_safety_discards_unfinished_calls_without_exposing_fragments() {
    let result=parse(&stream(vec![json!({"name":"lookup","partialArgs":[{"jsonPath":"$.city","stringValue":"private","willContinue":true}],"willContinue":true})],Some("SAFETY")),"model").unwrap();
    assert!(result.tool_calls.is_empty() && result.text.is_empty());
    assert_eq!(result.finish_reason.as_deref(), Some("content_filter"));
    assert_eq!(result.usage.unwrap().total_tokens, 5);
}

#[test]
fn vertex_express_preview_multiple_parts_repeat_metadata_without_splitting_call_identity() {
    let body = event(json!({"candidates":[{"content":{"parts":[
        {"functionCall":{"id":"explicit","name":"lookup","willContinue":true}},
        {"functionCall":{"id":"explicit","name":"lookup","partialArgs":[{"jsonPath":"$.x","stringValue":"A","willContinue":true}],"willContinue":true}},
        {"functionCall":{"partialArgs":[{"jsonPath":"$['x']","stringValue":"B"}]}}
    ]},"finishReason":"STOP"}]}));
    let result = parse(&body, "model").unwrap();
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].id.as_deref(), Some("explicit"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"x\":\"AB\"}")
    );
}
