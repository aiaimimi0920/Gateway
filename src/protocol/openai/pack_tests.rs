use serde_json::{json, Value};

use super::{normalize_chat_completions, pack_openai};

fn normalize(body: Value) -> crate::protocol::canonical::CanonicalRelayRequest {
    normalize_chat_completions(body).expect("fixture must normalize")
}

#[test]
fn streaming_pack_inserts_usage_options_without_overwriting_existing_values() {
    let missing = normalize(json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hello"}]
    }));
    let inserted = pack_openai(&missing, "gpt-4o", true);
    assert_eq!(inserted["stream_options"], json!({"include_usage": true}));

    let existing = normalize(json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hello"}],
        "stream_options": {"include_usage": false, "vendor_flag": 7}
    }));
    let preserved = pack_openai(&existing, "gpt-4o", true);
    assert_eq!(
        preserved["stream_options"],
        json!({"include_usage": false, "vendor_flag": 7})
    );

    let non_object = normalize(json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hello"}],
        "stream_options": "vendor-managed"
    }));
    assert_eq!(
        pack_openai(&non_object, "gpt-4o", true)["stream_options"],
        "vendor-managed"
    );
}

#[test]
fn reasoning_pack_prefers_explicit_completion_limit_and_omits_sampling() {
    let request = normalize(json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "reason"}],
        "max_tokens": 2000,
        "max_completion_tokens": 1500,
        "temperature": 0.2,
        "top_p": 0.8,
        "vendor_flag": true
    }));

    let packed = pack_openai(&request, "o3-mini", false);

    assert_eq!(packed["max_completion_tokens"], 1500);
    assert_eq!(packed["vendor_flag"], true);
    assert!(packed.get("max_tokens").is_none());
    assert!(packed.get("temperature").is_none());
    assert!(packed.get("top_p").is_none());
}

#[test]
fn openai_tool_choice_and_schema_keep_their_wire_shape() {
    let request = normalize(json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "weather"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "weather",
                "description": "Get weather",
                "parameters": {
                    "type": "object",
                    "properties": {"city": {"type": "string"}}
                }
            }
        }],
        "tool_choice": {"type": "function", "function": {"name": "weather"}}
    }));

    let packed = pack_openai(&request, "gpt-4o", false);

    assert_eq!(
        packed["tool_choice"],
        json!({"type": "function", "function": {"name": "weather"}})
    );
    assert_eq!(packed["tools"][0]["type"], "function");
    assert_eq!(packed["tools"][0]["function"]["name"], "weather");
    assert_eq!(
        packed["tools"][0]["function"]["parameters"]["properties"]["city"]["type"],
        "string"
    );
}

#[test]
fn mixed_content_and_tool_messages_keep_their_historic_wire_shape() {
    let request = normalize(json!({
        "model": "gpt-4o",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "inspect"},
                    {"type": "image_url", "image_url": {"url": "https://example.test/a.png", "detail": "low"}},
                    {"type": "vendor_part", "payload": 7}
                ]
            },
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_weather",
                    "type": "function",
                    "function": {"name": "weather", "arguments": "{\"city\":\"Hangzhou\"}"}
                }]
            },
            {
                "role": "tool",
                "name": "weather",
                "tool_call_id": "call_weather",
                "content": [
                    {"type": "text", "text": "sunny"},
                    {"type": "image_url", "image_url": {"url": "https://example.test/result.png"}}
                ]
            }
        ]
    }));

    let packed = pack_openai(&request, "gpt-4o", false);
    assert_eq!(
        packed["messages"][0]["content"][2],
        json!({
            "type": "vendor_part",
            "payload": 7
        })
    );
    assert_eq!(packed["messages"][1]["content"], "");
    assert_eq!(packed["messages"][1]["tool_calls"][0]["id"], "call_weather");
    assert_eq!(
        packed["messages"][2]["content"],
        "sunny\n[image omitted: https://example.test/result.png]"
    );
    assert_eq!(packed["messages"][2]["name"], "weather");
    assert_eq!(packed["messages"][2]["tool_call_id"], "call_weather");
}
