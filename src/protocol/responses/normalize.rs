use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    EndpointKind, MessageRole, ProtocolFamily,
};

use super::normalize_arguments_value;

/// Normalize an OpenAI Responses API request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_responses(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let previous_response_id = body
        .get("previous_response_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mut messages: Vec<CanonicalMessage> = Vec::new();

    if let Some(instructions) = body.get("instructions").and_then(|v| v.as_str()) {
        if !instructions.is_empty() {
            messages.push(CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: instructions.to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
    }

    match body.get("input") {
        Some(Value::String(s)) => {
            messages.push(CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text { text: s.clone() }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
        Some(Value::Array(arr)) => {
            for item in arr {
                messages.extend(normalize_responses_input_item(item)?);
            }
        }
        Some(other) => {
            messages.push(CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Raw {
                    value: other.clone(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
        None => {
            return Err(GatewayError::bad_request(
                "Responses API request missing `input` field",
            ));
        }
    }

    let mut extra = std::collections::HashMap::new();
    for key in &["temperature", "max_output_tokens", "top_p"] {
        if let Some(v) = body.get(*key) {
            extra.insert(key.to_string(), v.clone());
        }
    }

    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::Responses,
        requested_model: model,
        stream,
        messages,
        tools: parse_responses_tools(body.get("tools")),
        tool_choice: body.get("tool_choice").cloned(),
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id,
        explicit_session_key,
        extra,
    })
}

fn normalize_responses_input_item(item: &Value) -> Result<Vec<CanonicalMessage>, GatewayError> {
    let item_type = item.get("type").and_then(|value| value.as_str());
    match item_type {
        Some("function_call_output") | Some("custom_tool_call_output") => {
            Ok(vec![CanonicalMessage {
                role: MessageRole::Tool,
                content: parse_responses_output_content(item.get("output")),
                name: None,
                tool_call_id: item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_calls: vec![],
            }])
        }
        Some("function_call") | Some("custom_tool_call") => Ok(vec![CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![CanonicalToolCall {
                id: item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                call_type: item_type.unwrap_or("function").to_string(),
                name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                arguments: item.get("arguments").map(normalize_arguments_value),
                raw: std::collections::HashMap::new(),
            }],
        }]),
        _ => {
            let role_str = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let role = match role_str {
                "system" => MessageRole::System,
                "assistant" => MessageRole::Assistant,
                _ => MessageRole::User,
            };

            let content = match item.get("content") {
                Some(Value::String(s)) => vec![ContentPart::Text { text: s.clone() }],
                Some(Value::Array(parts)) => parts
                    .iter()
                    .filter_map(parse_responses_content_part)
                    .collect(),
                Some(other) => vec![ContentPart::Raw {
                    value: other.clone(),
                }],
                None => vec![],
            };

            Ok(vec![CanonicalMessage {
                role,
                content,
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }])
        }
    }
}

fn parse_responses_content_part(part: &Value) -> Option<ContentPart> {
    match part.get("type").and_then(|value| value.as_str()) {
        Some("input_text") | Some("output_text") | Some("text") => part
            .get("text")
            .and_then(|value| value.as_str())
            .map(|text| ContentPart::Text {
                text: text.to_string(),
            }),
        _ => Some(ContentPart::Raw {
            value: part.clone(),
        }),
    }
}

fn parse_responses_output_content(output: Option<&Value>) -> Vec<ContentPart> {
    match output {
        None | Some(Value::Null) => vec![],
        Some(Value::String(text)) => vec![ContentPart::Text { text: text.clone() }],
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(parse_responses_content_part)
            .collect(),
        Some(other) => vec![ContentPart::Raw {
            value: other.clone(),
        }],
    }
}

fn parse_responses_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|tool| {
            let is_function = tool.get("type").and_then(|value| value.as_str()) == Some("function");
            let function = if is_function {
                tool.get("function").or(Some(tool))
            } else {
                Some(tool)
            };

            CanonicalTool {
                tool_type: tool
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or("function")
                    .to_string(),
                name: function
                    .and_then(|value| value.get("name"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                description: function
                    .and_then(|value| value.get("description"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                input_schema: function
                    .and_then(|value| {
                        value
                            .get("parameters")
                            .or_else(|| value.get("input_schema"))
                    })
                    .cloned(),
                raw: tool
                    .as_object()
                    .map(|map| {
                        map.iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect()
}
