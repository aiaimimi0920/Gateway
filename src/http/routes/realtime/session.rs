//! Realtime session state and canonical request/item normalization.

use crate::protocol::canonical::CanonicalMessage;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::ContentPart;
use crate::protocol::canonical::EndpointKind;
use crate::protocol::canonical::MessageRole;
use crate::protocol::canonical::ProtocolFamily;
use crate::protocol::openai;
use crate::protocol::responses;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;

pub(super) struct RealtimeSession {
    pub(super) session_id: String,
    pub(super) model: String,
    pub(super) instructions: Option<String>,
    pub(super) tools: Vec<crate::protocol::canonical::CanonicalTool>,
    pub(super) tool_choice: Option<Value>,
    pub(super) messages: Vec<CanonicalMessage>,
}

impl RealtimeSession {
    pub(super) fn new(model: String) -> Self {
        Self {
            session_id: format!("sess_{}", uuid::Uuid::new_v4()),
            model,
            instructions: None,
            tools: Vec::new(),
            tool_choice: None,
            messages: Vec::new(),
        }
    }

    pub(super) fn build_request(&self, override_response: Option<&Value>) -> CanonicalRelayRequest {
        let mut messages = Vec::new();
        let response = override_response.unwrap_or(&Value::Null);
        let response_instructions = response
            .get("instructions")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let effective_instructions = response_instructions.or_else(|| self.instructions.clone());
        if let Some(instructions) = effective_instructions.clone() {
            if !instructions.trim().is_empty() {
                messages.push(CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text { text: instructions }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                });
            }
        }
        messages.extend(self.messages.clone());
        if messages
            .iter()
            .all(|message| !matches!(message.role, MessageRole::User))
        {
            if let Some(prompt) = effective_instructions {
                if !prompt.trim().is_empty() {
                    messages.push(CanonicalMessage {
                        // Some upstream families require at least one user turn even when
                        // OpenAI Realtime callers only provide `response.instructions`.
                        role: MessageRole::User,
                        content: vec![ContentPart::Text { text: prompt }],
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![],
                    });
                }
            }
        }

        let (tools, tool_choice) = parse_openai_session_tools(response)
            .unwrap_or_else(|| (self.tools.clone(), self.tool_choice.clone()));
        let requested_model = response
            .get("model")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| self.model.clone());

        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAiRealtime,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(requested_model),
            stream: true,
            messages,
            tools,
            tool_choice,
            reasoning: None,
            metadata: None,
            raw_body: response.clone(),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }
}

pub(super) fn parse_openai_session_tools(
    raw: &Value,
) -> Option<(
    Vec<crate::protocol::canonical::CanonicalTool>,
    Option<Value>,
)> {
    if raw.get("tools").is_none() && raw.get("tool_choice").is_none() {
        return None;
    }
    let prefers_responses_shape = raw
        .get("tools")
        .and_then(|value| value.as_array())
        .map(|tools| {
            tools.iter().any(|tool| {
                tool.get("function").is_none()
                    && tool
                        .get("type")
                        .and_then(|value| value.as_str())
                        .map(|value| value.eq_ignore_ascii_case("function"))
                        .unwrap_or(false)
                    && (tool.get("name").is_some()
                        || tool.get("description").is_some()
                        || tool.get("parameters").is_some())
            })
        })
        .unwrap_or(false);
    if prefers_responses_shape {
        let responses_synthetic = json!({
            "model": raw.get("model").cloned().unwrap_or_else(|| json!("gpt-5.4")),
            "input": [],
            "tools": raw.get("tools").cloned().unwrap_or_else(|| json!([])),
            "tool_choice": raw.get("tool_choice").cloned().unwrap_or(Value::Null),
        });
        if let Ok(request) = responses::normalize_responses(responses_synthetic) {
            return Some((request.tools, request.tool_choice));
        }
    }
    let chat_synthetic = json!({
        "model": raw.get("model").cloned().unwrap_or_else(|| json!("gpt-5.4")),
        "messages": [],
        "tools": raw.get("tools").cloned().unwrap_or_else(|| json!([])),
        "tool_choice": raw.get("tool_choice").cloned().unwrap_or(Value::Null),
    });
    if let Ok(request) = openai::normalize_chat_completions(chat_synthetic) {
        return Some((request.tools, request.tool_choice));
    }
    let responses_synthetic = json!({
        "model": raw.get("model").cloned().unwrap_or_else(|| json!("gpt-5.4")),
        "input": [],
        "tools": raw.get("tools").cloned().unwrap_or_else(|| json!([])),
        "tool_choice": raw.get("tool_choice").cloned().unwrap_or(Value::Null),
    });
    let request = responses::normalize_responses(responses_synthetic).ok()?;
    Some((request.tools, request.tool_choice))
}

pub(super) fn normalize_realtime_item(item: &Value) -> Option<CanonicalMessage> {
    match item
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("message")
    {
        "message" => {
            let role = match item
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or("user")
            {
                "assistant" => MessageRole::Assistant,
                "system" => MessageRole::System,
                _ => MessageRole::User,
            };
            let text = item
                .get("content")
                .and_then(|value| value.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| {
                            part.get("text")
                                .or_else(|| part.get("input_text"))
                                .and_then(|value| value.as_str())
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            Some(CanonicalMessage {
                role,
                content: if text.is_empty() {
                    Vec::new()
                } else {
                    vec![ContentPart::Text { text }]
                },
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            })
        }
        "function_call_output" => {
            let output = item.get("output").cloned().unwrap_or_else(|| json!(""));
            let content = if let Some(text) = output.as_str() {
                if let Ok(value) = serde_json::from_str::<Value>(text) {
                    vec![ContentPart::Json { value }]
                } else {
                    vec![ContentPart::Text {
                        text: text.to_string(),
                    }]
                }
            } else {
                vec![ContentPart::Json { value: output }]
            };
            Some(CanonicalMessage {
                role: MessageRole::Tool,
                content,
                name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_call_id: item
                    .get("call_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_calls: Vec::new(),
            })
        }
        _ => None,
    }
}
