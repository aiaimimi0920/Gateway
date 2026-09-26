//! Bedrock request normalization and raw-field preservation.

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalMessage;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::CanonicalTool;
use crate::protocol::canonical::CanonicalToolCall;
use crate::protocol::canonical::ContentPart;
use crate::protocol::canonical::EndpointKind;
use crate::protocol::canonical::MessageRole;
use crate::protocol::canonical::ProtocolFamily;
use crate::protocol::tool_choice;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;

pub fn normalize_converse(
    body: Value,
    path_model: Option<String>,
    stream: bool,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let requested_model = path_model.or_else(|| {
        body.get("modelId")
            .and_then(|value| value.as_str())
            .map(str::to_string)
    });

    let mut messages = Vec::new();
    if let Some(system) = body.get("system").and_then(|value| value.as_array()) {
        let text = system
            .iter()
            .filter_map(|entry| entry.get("text").and_then(|value| value.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            messages.push(CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text { text }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
    }

    let raw_messages = body
        .get("messages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| GatewayError::bad_request("missing or invalid `messages` array"))?;
    for raw_message in raw_messages {
        messages.extend(normalize_bedrock_message(raw_message));
    }

    let tools = parse_bedrock_tools(body.get("toolConfig").and_then(|value| value.get("tools")));
    let tool_choice = tool_choice::canonicalize_tool_choice(
        body.get("toolConfig")
            .and_then(|value| value.get("toolChoice")),
    );

    const KNOWN_FIELDS: &[&str] = &["modelId", "messages", "system", "toolConfig", "stream"];
    let mut extra = HashMap::new();
    if let Value::Object(map) = &body {
        for (key, value) in map {
            if !KNOWN_FIELDS.contains(&key.as_str()) {
                extra.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::BedrockConverse,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model,
        stream,
        messages,
        tools,
        tool_choice,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key: None,
        extra,
    })
}

fn normalize_bedrock_message(raw_message: &Value) -> Vec<CanonicalMessage> {
    let role = raw_message
        .get("role")
        .and_then(|value| value.as_str())
        .unwrap_or("user");
    let raw_content = raw_message
        .get("content")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let mut content = Vec::new();
    let mut tool_calls = Vec::new();
    let mut tool_results = Vec::new();

    for block in raw_content {
        if let Some(text) = block.get("text").and_then(|value| value.as_str()) {
            content.push(ContentPart::Text {
                text: text.to_string(),
            });
            continue;
        }
        if let Some(tool_use) = block.get("toolUse") {
            tool_calls.push(CanonicalToolCall {
                id: tool_use
                    .get("toolUseId")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                call_type: "function".to_string(),
                name: tool_use
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                arguments: tool_use.get("input").map(|value| value.to_string()),
                raw: HashMap::new(),
            });
            continue;
        }
        if let Some(tool_result) = block.get("toolResult") {
            let payload = tool_result
                .get("content")
                .and_then(|value| value.as_array())
                .and_then(|value| value.first())
                .and_then(|value| {
                    value
                        .get("json")
                        .cloned()
                        .or_else(|| value.get("text").cloned())
                })
                .unwrap_or_else(|| json!({}));
            tool_results.push(CanonicalMessage {
                role: MessageRole::Tool,
                content: vec![if let Some(text) = payload.as_str() {
                    ContentPart::Text {
                        text: text.to_string(),
                    }
                } else {
                    ContentPart::Json { value: payload }
                }],
                name: tool_result
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_call_id: tool_result
                    .get("toolUseId")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_calls: vec![],
            });
            continue;
        }

        if let Some(json_value) = block.get("json") {
            content.push(ContentPart::Json {
                value: json_value.clone(),
            });
            continue;
        }

        content.push(ContentPart::Raw {
            value: block.clone(),
        });
    }

    let canonical_role = if role == "assistant" {
        MessageRole::Assistant
    } else {
        MessageRole::User
    };
    let mut messages = Vec::new();
    if !content.is_empty() || !tool_calls.is_empty() {
        messages.push(CanonicalMessage {
            role: canonical_role,
            content,
            name: None,
            tool_call_id: None,
            tool_calls,
        });
    }
    messages.extend(tool_results);
    messages
}

fn parse_bedrock_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let Some(items) = raw.and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.get("toolSpec").cloned())
        .map(|tool_spec| CanonicalTool {
            tool_type: "function".to_string(),
            name: tool_spec
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            description: tool_spec
                .get("description")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            input_schema: tool_spec
                .get("inputSchema")
                .and_then(|value| value.get("json"))
                .cloned(),
            raw: tool_spec
                .as_object()
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}
