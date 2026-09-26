use serde_json::{json, Value};

use super::unpack_openai_response;

fn response_body(finish_reason: Value) -> Value {
    json!({
        "model": "gpt-4o",
        "choices": [{
            "message": {"role": "assistant", "content": "ok"},
            "finish_reason": finish_reason
        }]
    })
}

#[test]
fn usage_aliases_supply_cached_and_total_tokens() {
    let mut body = response_body(json!("stop"));
    body["usage"] = json!({
        "input_tokens": 11,
        "output_tokens": 7,
        "input_tokens_details": {"cached_tokens": 4}
    });

    let response = unpack_openai_response(&body).expect("response must unpack");
    let usage = response.usage.expect("usage must be present");

    assert_eq!(usage.prompt_tokens, 11);
    assert_eq!(usage.completion_tokens, 7);
    assert_eq!(usage.total_tokens, 18);
    assert_eq!(usage.cache_read_input_tokens, Some(4));
}

#[test]
fn primary_usage_fields_and_prompt_cache_details_take_precedence() {
    let mut body = response_body(json!("stop"));
    body["usage"] = json!({
        "prompt_tokens": 5,
        "input_tokens": 50,
        "completion_tokens": 3,
        "output_tokens": 30,
        "total_tokens": 99,
        "prompt_tokens_details": {"cached_tokens": 2},
        "input_tokens_details": {"cached_tokens": 20}
    });

    let response = unpack_openai_response(&body).expect("response must unpack");
    let usage = response.usage.expect("usage must be present");

    assert_eq!(usage.prompt_tokens, 5);
    assert_eq!(usage.completion_tokens, 3);
    assert_eq!(usage.total_tokens, 99);
    assert_eq!(usage.cache_read_input_tokens, Some(2));
}

#[test]
fn upstream_status_and_finish_reason_mapping_are_preserved() {
    for source in ["function_call", "tool_use"] {
        let mut body = response_body(json!(source));
        body["_upstream_status"] = json!(429);
        let response = unpack_openai_response(&body).expect("response must unpack");
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.upstream_status, Some(429));
    }

    let response =
        unpack_openai_response(&response_body(json!("  length  "))).expect("response must unpack");
    assert_eq!(response.finish_reason.as_deref(), Some("length"));
}

#[test]
fn invalid_choice_shapes_and_blank_finish_keep_their_historic_results() {
    assert!(unpack_openai_response(&json!({"choices": []})).is_err());
    assert!(unpack_openai_response(&json!({"choices": [{}]})).is_err());

    let response =
        unpack_openai_response(&response_body(json!("   "))).expect("response must unpack");
    assert_eq!(response.finish_reason, None);
}

#[test]
fn unpack_standard_response() {
    let body = json!({
        "id": "chatcmpl-123",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello there!"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });
    let resp = unpack_openai_response(&body).unwrap();
    assert_eq!(resp.text, "Hello there!");
    assert_eq!(resp.model, "gpt-4o");
    assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
    let usage = resp.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
}

#[test]
fn unpack_missing_choices_returns_error() {
    let body = json!({"model": "gpt-4o"});
    assert!(unpack_openai_response(&body).is_err());
}

#[test]
fn unpack_response_with_array_content() {
    let body = json!({
        "id": "chatcmpl-arr",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Hello"},
                    {"type": "text", "text": " world"}
                ]
            },
            "finish_reason": "stop"
        }]
    });
    let resp = unpack_openai_response(&body).unwrap();
    assert_eq!(resp.text, "Hello world");
    assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
}

#[test]
fn unpack_response_preserves_tool_calls() {
    let body = json!({
        "id": "chatcmpl-tc",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_abc",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"location\":\"NYC\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });
    let resp = unpack_openai_response(&body).unwrap();
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].id.as_deref(), Some("call_abc"));
    assert_eq!(resp.tool_calls[0].name.as_deref(), Some("get_weather"));
    assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
}

#[test]
fn unpack_response_unescapes_tool_call_argument_entities() {
    let body = json!({
        "id": "chatcmpl-escaped",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_html",
                    "type": "function",
                    "function": {
                        "name": "weather",
                        "arguments": "{&quot;city&quot;:&quot;Hangzhou&quot;}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });
    let resp = unpack_openai_response(&body).unwrap();
    assert_eq!(
        resp.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
}

#[test]
fn unpack_response_parses_legacy_function_call() {
    let body = json!({
        "id": "chatcmpl-fc",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "function_call": {
                    "name": "lookup_weather",
                    "arguments": {"city": "Shanghai"}
                }
            },
            "finish_reason": "function_call"
        }]
    });
    let resp = unpack_openai_response(&body).unwrap();
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].name.as_deref(), Some("lookup_weather"));
    assert_eq!(
        resp.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Shanghai\"}")
    );
    assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
}

#[test]
fn unpack_response_promotes_xml_tool_calls_from_text() {
    let body = json!({
        "id": "chatcmpl-xml",
        "model": "xop35qwen2b",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>"
            },
            "finish_reason": "stop"
        }]
    });
    let resp = unpack_openai_response(&body).unwrap();
    assert!(resp.text.is_empty());
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
}
