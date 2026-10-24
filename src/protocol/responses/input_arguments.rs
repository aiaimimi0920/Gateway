//! Normalize replayed function arguments without rebuilding opaque Responses history.
use serde_json::Value;

pub(crate) fn normalize_input_arguments(body: &mut Value) {
    let Some(items) = body.get_mut("input").and_then(Value::as_array_mut) else {
        return;
    };
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            continue;
        }
        // Keep valid strings byte-for-byte, including whitespace and non-JSON
        // tool syntax. Never rewrite custom-tool inputs, outputs or absent fields.
        if let Some(arguments) = item.get_mut("arguments").filter(|value| !value.is_string()) {
            *arguments = Value::String(super::normalize_arguments_value(arguments));
        }
    }
}
