//! Chat and Responses use different envelopes for the same tool identity.
//! Custom inputs are opaque strings: never JSON-normalize or HTML-decode them.
use super::canonical::{CanonicalRelayRequest, CanonicalTool, CanonicalToolCall};
use serde_json::{json, Map, Value};

pub(super) fn is_custom(call: &CanonicalToolCall) -> bool {
    matches!(call.call_type.as_str(), "custom" | "custom_tool_call")
}

/// One linear index per request, including outputs resumed by previous_response_id.
pub(super) fn output_kinds(req: &CanonicalRelayRequest) -> std::collections::HashMap<&str, bool> {
    let mut kinds = std::collections::HashMap::new();
    for call in req.messages.iter().flat_map(|message| &message.tool_calls) {
        if let Some(id) = call.id.as_deref() {
            kinds.insert(id, is_custom(call));
        }
    }
    if let Some(items) = req.raw_body.get("input").and_then(Value::as_array) {
        for item in items {
            let kind = item.get("type").and_then(Value::as_str);
            if matches!(
                kind,
                Some("function_call_output" | "custom_tool_call_output")
            ) {
                if let Some(id) = item.get("call_id").and_then(Value::as_str) {
                    kinds
                        .entry(id)
                        .or_insert(kind == Some("custom_tool_call_output"));
                }
            }
        }
    }
    kinds
}

pub(super) fn chat_call(call: &CanonicalToolCall) -> Value {
    if is_custom(call) {
        json!({"id":call.id,"type":"custom","custom":{"name":call.name,"input":call.arguments}})
    } else {
        json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.arguments}})
    }
}

pub(super) fn responses_call(call: &CanonicalToolCall) -> Value {
    let custom = is_custom(call);
    let mut item = json!({
        "type":if custom {"custom_tool_call"} else {"function_call"},
        "call_id":call.id.as_deref().unwrap_or("call_auto"),
        "name":call.name.as_deref().unwrap_or("tool"),
    });
    item[if custom { "input" } else { "arguments" }] = json!(call
        .arguments
        .as_deref()
        .unwrap_or(if custom { "" } else { "{}" }));
    for key in ["namespace", "id", "status"] {
        if let Some(value) = call.raw.get(key) {
            item[key] = value.clone();
        }
    }
    item
}

fn definition(tool: &CanonicalTool, preserve_raw: bool) -> Map<String, Value> {
    let nested = if tool.tool_type == "custom" {
        "custom"
    } else {
        "function"
    };
    let mut fields = if preserve_raw {
        tool.raw
            .get(nested)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_else(|| {
                tool.raw
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
    } else {
        Map::new()
    };
    for key in ["type", "function", "custom"] {
        fields.remove(key);
    }
    if let Some(name) = &tool.name {
        fields.insert("name".into(), json!(name));
    }
    if let Some(description) = &tool.description {
        fields.insert("description".into(), json!(description));
    }
    if let Some(schema) = &tool.input_schema {
        fields.insert("parameters".into(), schema.clone());
    }
    fields
}

pub(super) fn chat_tool(tool: &CanonicalTool, preserve_raw: bool) -> Value {
    if !matches!(tool.tool_type.as_str(), "function" | "custom") {
        return Value::Object(
            tool.raw
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
    }
    let kind = tool.tool_type.as_str();
    let mut value = json!({"type":kind});
    // Only the same protocol family may reuse raw wire fields. Foreign schemas
    // and cache-control envelopes must pass through the canonical fields above.
    value[kind] = Value::Object(definition(tool, preserve_raw));
    value
}

pub(super) fn responses_tool(tool: &CanonicalTool) -> Value {
    let mut fields = definition(tool, true);
    fields.insert("type".into(), json!(tool.tool_type));
    Value::Object(fields)
}

fn selector_to_chat(value: &Value) -> Value {
    let Some(kind @ ("function" | "custom")) = value.get("type").and_then(Value::as_str) else {
        return value.clone();
    };
    if value.get(kind).is_some() {
        return value.clone();
    }
    let mut config = value.as_object().cloned().unwrap_or_default();
    config.remove("type");
    let mut result = json!({"type":kind});
    result[kind] = Value::Object(config);
    result
}

fn selector_to_responses(value: &Value) -> Value {
    let Some(kind @ ("function" | "custom")) = value.get("type").and_then(Value::as_str) else {
        return value.clone();
    };
    let Some(mut config) = value.get(kind).and_then(Value::as_object).cloned() else {
        return value.clone();
    };
    config.insert("type".into(), json!(kind));
    Value::Object(config)
}

pub(super) fn chat_choice(value: &Value) -> Option<Value> {
    match value.get("type").and_then(Value::as_str)? {
        "function" | "custom" => Some(selector_to_chat(value)),
        "allowed_tools" => {
            let source = value.get("allowed_tools").unwrap_or(value);
            let tools = source
                .get("tools")?
                .as_array()?
                .iter()
                .map(selector_to_chat)
                .collect::<Vec<_>>();
            Some(
                json!({"type":"allowed_tools","allowed_tools":{"mode":source.get("mode").cloned().unwrap_or(json!("auto")),"tools":tools}}),
            )
        }
        _ => None,
    }
}

pub(super) fn responses_choice(value: &Value) -> Option<Value> {
    match value.get("type").and_then(Value::as_str)? {
        "function" | "custom" => Some(selector_to_responses(value)),
        "allowed_tools" => {
            let source = value.get("allowed_tools").unwrap_or(value);
            let tools = source
                .get("tools")?
                .as_array()?
                .iter()
                .map(selector_to_responses)
                .collect::<Vec<_>>();
            Some(
                json!({"type":"allowed_tools","mode":source.get("mode").cloned().unwrap_or(json!("auto")),"tools":tools}),
            )
        }
        _ => None,
    }
}
