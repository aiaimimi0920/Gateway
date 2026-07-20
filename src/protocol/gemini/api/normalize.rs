use std::collections::HashMap;

use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::tool_choice;

use super::parse_gemini_tool_call;

pub fn normalize_generate_content(
    body: Value,
    path_model: Option<String>,
    stream: bool,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let requested_model = path_model.or_else(|| {
        body.get("model")
            .and_then(|value| value.as_str())
            .map(str::to_string)
    });

    let mut messages = Vec::new();
    if let Some(system_instruction) = body.get("system_instruction") {
        let system_text = extract_parts_text(system_instruction.get("parts"));
        if !system_text.is_empty() {
            messages.push(CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text { text: system_text }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
    }

    let raw_contents = body
        .get("contents")
        .and_then(|value| value.as_array())
        .ok_or_else(|| GatewayError::bad_request("missing or invalid `contents` array"))?;

    for raw_message in raw_contents {
        let normalized = normalize_gemini_message(raw_message, &messages);
        messages.extend(normalized);
    }

    let tools = parse_gemini_tools(body.get("tools"));
    let tool_choice = tool_choice::canonicalize_tool_choice(
        body.get("toolConfig")
            .and_then(|value| value.get("functionCallingConfig")),
    );

    const KNOWN_FIELDS: &[&str] = &[
        "model",
        "contents",
        "system_instruction",
        "tools",
        "toolConfig",
        "thinkingConfig",
    ];
    let mut extra = HashMap::new();
    if let Value::Object(map) = &body {
        for (key, value) in map {
            if !KNOWN_FIELDS.contains(&key.as_str()) {
                extra.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::GeminiGenerateContent,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model,
        stream,
        messages,
        tools,
        tool_choice,
        reasoning: body
            .get("thinkingConfig")
            .filter(|value| !value.is_null())
            .cloned(),
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key: None,
        extra,
    })
}

fn normalize_gemini_message(
    raw_message: &Value,
    history: &[CanonicalMessage],
) -> Vec<CanonicalMessage> {
    let role = raw_message
        .get("role")
        .and_then(|value| value.as_str())
        .unwrap_or("user");
    let raw_parts = raw_message
        .get("parts")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let mut content = Vec::new();
    let mut tool_calls = Vec::new();
    let mut tool_results = Vec::new();

    for part in raw_parts {
        if let Some(text) = part.get("text").and_then(|value| value.as_str()) {
            content.push(ContentPart::Text {
                text: text.to_string(),
            });
            continue;
        }

        if let Some(function_call) = part
            .get("functionCall")
            .or_else(|| part.get("function_call"))
        {
            tool_calls.push(parse_gemini_tool_call(function_call));
            continue;
        }

        if let Some(function_response) = part
            .get("functionResponse")
            .or_else(|| part.get("function_response"))
        {
            tool_results.push(parse_gemini_tool_result(
                function_response,
                &tool_calls,
                history,
            ));
            continue;
        }

        if let Some(file_data) = part.get("fileData").and_then(|value| value.as_object()) {
            let file_uri = file_data
                .get("fileUri")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let mime_type = file_data
                .get("mimeType")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if mime_type.starts_with("image/")
                || file_uri.ends_with(".png")
                || file_uri.ends_with(".jpg")
                || file_uri.ends_with(".jpeg")
                || file_uri.ends_with(".webp")
            {
                content.push(ContentPart::ImageUrl {
                    image_url: file_uri.to_string(),
                    detail: None,
                });
            } else {
                content.push(ContentPart::Raw {
                    value: part.clone(),
                });
            }
            continue;
        }

        if part.get("inlineData").is_some() || part.get("inline_data").is_some() {
            content.push(ContentPart::Raw {
                value: part.clone(),
            });
            continue;
        }

        content.push(ContentPart::Raw {
            value: part.clone(),
        });
    }

    let mut messages = Vec::new();
    let canonical_role = if role == "model" {
        MessageRole::Assistant
    } else {
        MessageRole::User
    };
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

fn parse_gemini_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let Some(items) = raw.and_then(|value| value.as_array()) else {
        return Vec::new();
    };

    let mut tools = Vec::new();
    for item in items {
        let declarations = item
            .get("functionDeclarations")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        for declaration in declarations {
            tools.push(CanonicalTool {
                tool_type: "function".to_string(),
                name: declaration
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                description: declaration
                    .get("description")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                input_schema: declaration.get("parameters").cloned(),
                raw: declaration
                    .as_object()
                    .map(|map| {
                        map.iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }
    tools
}

fn parse_gemini_tool_result(
    raw: &Value,
    current_tool_calls: &[CanonicalToolCall],
    history: &[CanonicalMessage],
) -> CanonicalMessage {
    let payload = raw
        .get("response")
        .or_else(|| raw.get("output"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let inferred_name = raw
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let inferred_call_id = raw
        .get("id")
        .or_else(|| raw.get("call_id"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            inferred_name
                .as_deref()
                .and_then(|name| find_tool_call_id_by_name(current_tool_calls, name))
        })
        .or_else(|| {
            inferred_name
                .as_deref()
                .and_then(|name| find_tool_call_id_in_history(history, name))
        });
    CanonicalMessage {
        role: MessageRole::Tool,
        content: vec![ContentPart::Json { value: payload }],
        name: inferred_name,
        tool_call_id: inferred_call_id,
        tool_calls: vec![],
    }
}

fn find_tool_call_id_by_name(tool_calls: &[CanonicalToolCall], name: &str) -> Option<String> {
    tool_calls
        .iter()
        .rev()
        .find(|tool_call| tool_call.name.as_deref() == Some(name))
        .and_then(|tool_call| tool_call.id.clone())
}

fn find_tool_call_id_in_history(history: &[CanonicalMessage], name: &str) -> Option<String> {
    history.iter().rev().find_map(|message| {
        message
            .tool_calls
            .iter()
            .rev()
            .find(|tool_call| tool_call.name.as_deref() == Some(name))
            .and_then(|tool_call| tool_call.id.clone())
    })
}

fn extract_parts_text(raw_parts: Option<&Value>) -> String {
    raw_parts
        .and_then(|value| value.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(|value| value.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ignores_null_thinking_config_in_extra_and_reasoning() {
        let req = normalize_generate_content(
            json!({
                "model": "gemini-2.5-pro",
                "contents": [{"role": "user", "parts": [{"text": "hi"}]}],
                "thinkingConfig": null,
            }),
            None,
            true,
        )
        .unwrap();

        assert!(req.reasoning.is_none());
        assert!(!req.extra.contains_key("thinkingConfig"));
    }

    #[test]
    fn normalize_preserves_non_image_file_data_as_raw() {
        let req = normalize_generate_content(
            json!({
                "model": "gemini-2.5-pro",
                "contents": [{
                    "role": "user",
                    "parts": [{
                        "fileData": {
                            "fileUri": "https://example.com/manual.pdf",
                            "mimeType": "application/pdf"
                        }
                    }]
                }]
            }),
            None,
            false,
        )
        .unwrap();

        assert!(matches!(
            req.messages[0].content[0],
            ContentPart::Raw { .. }
        ));
    }

    #[test]
    fn normalize_preserves_inline_data_as_raw() {
        let req = normalize_generate_content(
            json!({
                "model": "gemini-2.5-pro",
                "contents": [{
                    "role": "user",
                    "parts": [{
                        "inlineData": {
                            "mimeType": "audio/wav",
                            "data": "UklGRiQAAABXQVZFZm10"
                        }
                    }]
                }]
            }),
            None,
            false,
        )
        .unwrap();

        assert!(matches!(
            req.messages[0].content[0],
            ContentPart::Raw { .. }
        ));
    }
}
