use serde_json::Value;

use crate::protocol::canonical::CanonicalToolCall;

pub(super) fn parse_openai_message_tool_calls(message: &Value) -> Vec<CanonicalToolCall> {
    let tool_calls = parse_openai_tool_calls(message.get("tool_calls"));
    if !tool_calls.is_empty() {
        return tool_calls;
    }
    parse_openai_legacy_function_call(message.get("function_call"))
}

fn parse_openai_legacy_function_call(raw: Option<&Value>) -> Vec<CanonicalToolCall> {
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
        arguments: raw.get("arguments").map(normalize_tool_arguments),
        raw: std::collections::HashMap::new(),
    }]
}

fn parse_openai_tool_calls(raw: Option<&Value>) -> Vec<CanonicalToolCall> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|tc| {
            let fn_obj = tc.get("function");
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
                    .and_then(|f| f.get("arguments"))
                    .map(normalize_tool_arguments),
                raw: std::collections::HashMap::new(),
            }
        })
        .collect()
}

fn normalize_tool_arguments(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        decode_provider_tool_argument_entities(text)
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
