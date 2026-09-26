use serde_json::{json, Map, Value};

use crate::protocol::anthropic::PromptCacheTelemetry;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalTool};

use super::request_contents::{build_contents, ensure_alternating_roles};
use super::value_to_string;

pub fn pack_accio(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = Map::new();
    let client_cache_control = find_first_cache_control(&req.raw_body).cloned();
    body.insert("model".into(), json!(model));
    body.insert("incremental".into(), Value::Bool(stream));
    body.insert(
        "request_id".into(),
        json!(request_string(req, &["request_id", "requestId"])
            .unwrap_or_else(|| format!("user-{}", unix_millis()))),
    );
    body.insert(
        "message_id".into(),
        json!(
            request_string(req, &["message_id", "messageId"]).unwrap_or_else(|| {
                format!(
                    "AI_AccioWork_{}_{}",
                    uuid::Uuid::new_v4().as_simple(),
                    unix_millis()
                )
            })
        ),
    );
    body.insert(
        "max_output_tokens".into(),
        json!(request_u64(
            req,
            &["max_output_tokens", "max_completion_tokens", "max_tokens"]
        )
        .unwrap_or(8192)),
    );
    body.insert(
        "contents".into(),
        Value::Array(ensure_alternating_roles(build_contents(req))),
    );
    body.insert("properties".into(), Value::Object(build_properties(req)));

    if let Some(system) = req.system_message().filter(|v| !v.is_empty()) {
        body.insert("system_instruction".into(), json!(system));
    }
    if let Some(value) = request_value(req, &["temperature"]) {
        body.insert("temperature".into(), value.clone());
    }
    if let Some(value) = request_value(req, &["top_p"]) {
        body.insert("top_p".into(), value.clone());
    }
    if let Some(value) = request_value(req, &["response_format"]) {
        body.insert("response_format".into(), value.clone());
    }

    let stops = normalize_stop_sequences(request_value(req, &["stop_sequences", "stop"]));
    if !stops.is_empty() {
        body.insert(
            "stop_sequences".into(),
            Value::Array(stops.into_iter().map(Value::String).collect()),
        );
    }

    apply_thinking(&mut body, req);

    if !req.tools.is_empty() {
        body.insert(
            "tools".into(),
            Value::Array(req.tools.iter().map(pack_tool).collect()),
        );
    }

    if let Some(cache_control) = client_cache_control {
        body.insert("cache_control".into(), cache_control);
    } else if should_auto_apply_cache_control(req, model) {
        body.insert("cache_control".into(), json!({ "type": "ephemeral" }));
    }

    for (aliases, keys) in [
        (
            &["session_key", "sessionKey"][..],
            &["session_key", "sessionKey"][..],
        ),
        (
            &["conversation_id", "conversationId"][..],
            &["conversation_id", "conversationId"][..],
        ),
        (
            &["conversation_name", "conversationName"][..],
            &["conversation_name", "conversationName"][..],
        ),
    ] {
        if let Some(value) = request_value(req, aliases) {
            for key in keys {
                body.entry((*key).to_string())
                    .or_insert_with(|| value.clone());
            }
        }
    }

    if let Some(value) = request_value(req, &["empid", "empId", "accountId", "account_id"]) {
        body.entry("empid".to_string())
            .or_insert_with(|| value.clone());
    }
    if let Some(value) = request_value(req, &["tenant", "tenantId", "tenant_id"]) {
        body.entry("tenant".to_string())
            .or_insert_with(|| value.clone());
    }
    if let Some(value) = request_value(req, &["iaiTag", "iai_tag"]) {
        body.entry("iaiTag".to_string())
            .or_insert_with(|| value.clone());
    }

    for (key, value) in &req.extra {
        if is_handled_extra_key(key) {
            continue;
        }
        body.entry(key.clone()).or_insert_with(|| value.clone());
    }

    Value::Object(body)
}

pub fn inspect_prompt_cache_telemetry(
    req: &CanonicalRelayRequest,
    model: &str,
) -> PromptCacheTelemetry {
    let client_has_cache_control = find_first_cache_control(&req.raw_body).is_some();
    let auto_cache_applied =
        !client_has_cache_control && should_auto_apply_cache_control(req, model);
    PromptCacheTelemetry {
        client_has_cache_control,
        auto_cache_applied,
    }
}

fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn request_value<'a>(req: &'a CanonicalRelayRequest, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = req.extra.get(*key) {
            return Some(value);
        }
        if let Some(value) = req.raw_body.get(*key) {
            return Some(value);
        }
    }
    None
}

fn request_string(req: &CanonicalRelayRequest, keys: &[&str]) -> Option<String> {
    request_value(req, keys).and_then(value_to_string)
}

fn request_u64(req: &CanonicalRelayRequest, keys: &[&str]) -> Option<u64> {
    request_value(req, keys).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|v| v.max(0) as u64))
            .or_else(|| value.as_str().and_then(|v| v.parse::<u64>().ok()))
    })
}

fn normalize_stop_sequences(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![text.to_string()]
            }
        }
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::trim).map(str::to_string))
            .filter(|item| !item.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn build_properties(req: &CanonicalRelayRequest) -> Map<String, Value> {
    let mut properties = request_value(req, &["properties"])
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    if let Some(metadata) = &req.metadata {
        properties
            .entry("canonical_metadata".to_string())
            .or_insert_with(|| json!(metadata));
    }
    if let Some(metadata) = request_value(req, &["metadata"]) {
        properties
            .entry("openai_metadata".to_string())
            .or_insert_with(|| metadata.clone());
    }
    if let Some(user) = &req.explicit_session_key {
        properties
            .entry("openai_user".to_string())
            .or_insert_with(|| json!(user));
    }
    if let Some(tool_choice) = &req.tool_choice {
        properties
            .entry("openai_tool_choice".to_string())
            .or_insert_with(|| tool_choice.clone());
    }
    if let Some(reasoning) = &req.reasoning {
        properties
            .entry("openai_reasoning".to_string())
            .or_insert_with(|| reasoning.clone());
    }
    if let Some(previous_response_id) = &req.previous_response_id {
        properties
            .entry("openai_previous_response_id".to_string())
            .or_insert_with(|| json!(previous_response_id));
    }
    for (aliases, key) in [
        (&["session_id", "sessionId"][..], "openai_session_id"),
        (
            &["conversation_id", "conversationId"][..],
            "openai_conversation_id",
        ),
        (&["text"][..], "openai_text"),
        (&["truncation"][..], "openai_truncation"),
        (&["include"][..], "openai_include"),
    ] {
        if let Some(value) = request_value(req, aliases) {
            properties
                .entry(key.to_string())
                .or_insert_with(|| value.clone());
        }
    }
    properties
}

fn apply_thinking(body: &mut Map<String, Value>, req: &CanonicalRelayRequest) {
    let reasoning = req
        .reasoning
        .as_ref()
        .or_else(|| request_value(req, &["thinking"]));
    let Some(reasoning) = reasoning.and_then(|v| v.as_object()) else {
        return;
    };
    let thinking_type = reasoning
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("enabled")
        .to_ascii_lowercase();
    if thinking_type != "enabled" {
        return;
    }
    body.insert("include_thoughts".into(), Value::Bool(true));
    if let Some(budget) = reasoning
        .get("budget_tokens")
        .or_else(|| reasoning.get("budgetTokens"))
        .and_then(|v| v.as_u64())
    {
        body.insert("thinking_budget".into(), json!(budget));
        body.insert(
            "thinking_level".into(),
            json!(if budget >= 12_000 {
                "high"
            } else if budget >= 4_000 {
                "medium"
            } else {
                "low"
            }),
        );
        return;
    }
    let level = reasoning
        .get("level")
        .or_else(|| reasoning.get("effort"))
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "high".into());
    body.insert(
        "thinking_level".into(),
        json!(match level.as_str() {
            "low" | "medium" | "high" => level,
            _ => "high".into(),
        }),
    );
}

fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "name": tool.name.clone().unwrap_or_default(),
        "description": tool.description.clone().unwrap_or_default(),
        "parameters_json": serde_json::to_string(
            tool.input_schema
                .as_ref()
                .unwrap_or(&json!({"type": "object", "properties": {}}))
        ).unwrap_or_else(|_| "{}".into())
    })
}

fn is_claude_model(model: &str) -> bool {
    model.to_ascii_lowercase().contains("claude")
}

fn should_auto_apply_cache_control(req: &CanonicalRelayRequest, model: &str) -> bool {
    is_claude_model(model) && !auto_cache_disabled(req)
}

fn auto_cache_disabled(req: &CanonicalRelayRequest) -> bool {
    for key in ["gateway_auto_cache", "neuro_auto_cache"] {
        let Some(value) = req.raw_body.get(key).or_else(|| req.extra.get(key)) else {
            continue;
        };
        match value {
            Value::Bool(flag) => {
                if !flag {
                    return true;
                }
            }
            Value::String(text) => {
                let normalized = text.trim().to_ascii_lowercase();
                if matches!(normalized.as_str(), "false" | "0" | "off" | "no") {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn find_first_cache_control(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if let Some(cache_control) = map.get("cache_control") {
                return Some(cache_control);
            }
            map.values().find_map(find_first_cache_control)
        }
        Value::Array(items) => items.iter().find_map(find_first_cache_control),
        _ => None,
    }
}

fn is_handled_extra_key(key: &str) -> bool {
    matches!(
        key,
        "max_tokens"
            | "max_completion_tokens"
            | "max_output_tokens"
            | "temperature"
            | "top_p"
            | "response_format"
            | "stop"
            | "stop_sequences"
            | "properties"
            | "metadata"
            | "tool_choice"
            | "session_id"
            | "sessionId"
            | "conversation_id"
            | "conversationId"
            | "conversation_name"
            | "conversationName"
            | "request_id"
            | "requestId"
            | "message_id"
            | "messageId"
            | "accountId"
            | "account_id"
            | "empid"
            | "tenant"
            | "tenantId"
            | "tenant_id"
            | "iaiTag"
            | "iai_tag"
            | "gateway_auto_cache"
            | "neuro_auto_cache"
            | "text"
            | "include"
            | "truncation"
            | "thinking"
    )
}
