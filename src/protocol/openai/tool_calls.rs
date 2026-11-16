use serde_json::Value;

use crate::protocol::canonical::CanonicalToolCall;

pub(super) fn parse_openai_message_tool_calls(message: &Value) -> Vec<CanonicalToolCall> {
    parse_message_calls(message, true)
}

/// Signed Google JSON arguments cannot undergo the legacy provider HTML-entity repair.
pub(super) fn parse_openai_message_tool_calls_lossless(message: &Value) -> Vec<CanonicalToolCall> {
    parse_message_calls(message, false)
}

fn parse_message_calls(message: &Value, decode_entities: bool) -> Vec<CanonicalToolCall> {
    let tool_calls = parse_openai_tool_calls(message.get("tool_calls"), decode_entities);
    if !tool_calls.is_empty() {
        return tool_calls;
    }
    parse_openai_legacy_function_call(message.get("function_call"), decode_entities)
}

fn parse_openai_legacy_function_call(
    raw: Option<&Value>,
    decode_entities: bool,
) -> Vec<CanonicalToolCall> {
    let Some(raw) = raw else {
        return vec![];
    };

    if !raw.is_object() {
        return vec![];
    }

    vec![CanonicalToolCall {
        id: raw
            .get("id")
            .or_else(|| raw.get("call_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        call_type: "function".to_string(),
        name: raw.get("name").and_then(|v| v.as_str()).map(str::to_string),
        arguments: raw
            .get("arguments")
            .map(|v| normalize_tool_arguments(v, decode_entities)),
        raw: std::collections::HashMap::new(),
    }]
}

fn parse_openai_tool_calls(raw: Option<&Value>, decode_entities: bool) -> Vec<CanonicalToolCall> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|tc| {
            let custom = tc.get("type").and_then(Value::as_str) == Some("custom");
            let fn_obj = tc.get(if custom { "custom" } else { "function" });
            CanonicalToolCall {
                id: tc.get("id").and_then(|v| v.as_str()).map(str::to_string),
                call_type: tc
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string(),
                name: fn_obj
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                arguments: fn_obj
                    .and_then(|f| f.get(if custom { "input" } else { "arguments" }))
                    .map(|value| {
                        if custom {
                            value
                                .as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| value.to_string())
                        } else {
                            normalize_tool_arguments(value, decode_entities)
                        }
                    }),
                raw: std::collections::HashMap::new(),
            }
        })
        .collect()
}

fn normalize_tool_arguments(value: &Value, decode_entities: bool) -> String {
    if let Some(text) = value.as_str() {
        if decode_entities {
            decode_provider_tool_argument_entities(text)
        } else {
            text.to_owned()
        }
    } else {
        serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
    }
}

fn decode_provider_tool_argument_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
