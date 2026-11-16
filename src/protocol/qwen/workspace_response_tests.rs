//! Validation is product-local; valid truncated/filtered/tool responses retain their actual semantics.
use super::*;
use serde_json::json;

fn completed() -> Value {
    json!({"model":"qwen-synthetic","output":[],"choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10,"prompt_tokens_details":{"cached_tokens":3}}})
}
#[test]
fn qwen_workspace_response_uses_chat_choices_not_responses_extension() {
    let result = unpack(&completed()).unwrap();
    assert_eq!(result.text, "OK");
    let usage = result.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 8);
    assert_eq!(usage.cache_read_input_tokens, Some(3));
}
#[test]
fn qwen_workspace_response_rejects_malformed_shapes_and_unfinished_choices() {
    let original = completed();
    let mut cases = vec![Value::Null, json!({"output":[]}), json!({"choices":[]})];
    for message in [
        Value::Null,
        json!({}),
        json!({"role":"user","content":"OK"}),
        json!({"role":"assistant","content":5}),
    ] {
        let mut body = original.clone();
        body["choices"][0]["message"] = message;
        cases.push(body);
    }
    for terminal in [Value::Null, json!(""), json!("unknown"), json!(5)] {
        let mut body = original.clone();
        body["choices"][0]["finish_reason"] = terminal;
        cases.push(body);
    }
    let mut missing = original.clone();
    missing["choices"][0]
        .as_object_mut()
        .unwrap()
        .remove("finish_reason");
    cases.push(missing);
    let mut error = original.clone();
    error["error"] = json!({"message":"private-upstream-detail"});
    cases.push(error);
    let mut tail = original;
    tail["choices"]
        .as_array_mut()
        .unwrap()
        .push(json!({"message":{}}));
    cases.push(tail);
    for body in cases {
        let error = unpack(&body).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("invalid_qwen_workspace_response")
        );
        assert!(!error.message.contains("private-upstream-detail"));
        assert!(!error.retryable);
    }
}
#[test]
fn qwen_workspace_response_preserves_length_filter_and_empty_string_semantics() {
    for (terminal, content) in [
        ("length", Value::Null),
        ("content_filter", Value::Null),
        ("stop", json!("")),
    ] {
        let mut body = completed();
        body["choices"][0]["finish_reason"] = json!(terminal);
        body["choices"][0]["message"]["content"] = content;
        let result = unpack(&body).unwrap();
        assert_eq!(result.finish_reason.as_deref(), Some(terminal));
        assert!(result.text.is_empty());
    }
}
#[test]
fn qwen_workspace_response_keeps_function_tools_and_legacy_compatibility() {
    for legacy in [false, true] {
        let mut body = completed();
        let message = &mut body["choices"][0]["message"];
        message["content"] = Value::Null;
        if legacy {
            message["function_call"] = json!({"name":"lookup","arguments":"{\"q\":\"x\"}"});
        } else {
            message["tool_calls"] = json!([{"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{\"q\":\"x\"}"}}]);
        }
        body["choices"][0]["finish_reason"] = json!(if legacy {
            "function_call"
        } else {
            "tool_calls"
        });
        let result = unpack(&body).unwrap();
        assert_eq!(result.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name.as_deref(), Some("lookup"));
        assert_eq!(
            result.tool_calls[0].arguments.as_deref(),
            Some("{\"q\":\"x\"}")
        );
    }
}
#[test]
fn qwen_workspace_response_rejects_silently_dropped_or_malformed_tools() {
    for tools in [
        json!({}),
        json!([]),
        json!([{}]),
        json!([{"id":"call-1","type":"function","function":{"name":"lookup","arguments":{}}}]),
    ] {
        let mut body = completed();
        body["choices"][0]["finish_reason"] = json!("tool_calls");
        body["choices"][0]["message"]["tool_calls"] = tools;
        assert!(unpack(&body).is_err());
    }
}
#[test]
fn qwen_workspace_response_validates_optional_usage_without_inventing_tokens() {
    let mut body = completed();
    body.as_object_mut().unwrap().remove("usage");
    assert!(unpack(&body).unwrap().usage.is_none());
    body["usage"] = json!({"prompt_tokens":8,"completion_tokens":2});
    assert_eq!(unpack(&body).unwrap().usage.unwrap().total_tokens, 10);
    for usage in [
        json!({}),
        json!({"prompt_tokens":-1,"completion_tokens":2}),
        json!({"prompt_tokens":u64::MAX,"completion_tokens":1}),
        json!({"prompt_tokens":8,"completion_tokens":2,"total_tokens":9}),
        json!({"prompt_tokens":8,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":9}}),
        json!({"prompt_tokens":8,"completion_tokens":2,"input_tokens_details":{"cached_tokens":999}}),
        json!({"prompt_tokens":8,"completion_tokens":2,"prompt_tokens_details":{},"input_tokens_details":{"cached_tokens":999}}),
    ] {
        body["usage"] = usage;
        assert!(unpack(&body).is_err());
    }
    body["usage"] =
        json!({"prompt_tokens":8,"completion_tokens":2,"input_tokens_details":{"cached_tokens":3}});
    assert_eq!(
        unpack(&body)
            .unwrap()
            .usage
            .unwrap()
            .cache_read_input_tokens,
        Some(3)
    );
    body["usage"]["prompt_tokens_details"] = json!({"cached_tokens":4});
    body["usage"]["input_tokens_details"]["cached_tokens"] = json!(999);
    assert_eq!(
        unpack(&body)
            .unwrap()
            .usage
            .unwrap()
            .cache_read_input_tokens,
        Some(4)
    );
}
