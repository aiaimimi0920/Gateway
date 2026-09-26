//! Gemini Live wire payloads; socket and session lifecycle stay in the route.

use serde_json::{json, Value};

use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole};

use super::GeminiLiveSession;

fn pack_gemini_live_message(message: &CanonicalMessage) -> Value {
    match message.role {
        MessageRole::Tool => json!({
            "role": "user",
            "parts": [{
                "functionResponse": {
                    "id": message.tool_call_id,
                    "name": message.name,
                    "response": match message.content.first() {
                        Some(ContentPart::Json { value }) => value.clone(),
                        _ => json!({ "content": message.text_content() }),
                    }
                }
            }]
        }),
        MessageRole::Assistant => {
            let mut parts = Vec::new();
            if !message.text_content().is_empty() {
                parts.push(json!({"text": message.text_content()}));
            }
            for tool_call in &message.tool_calls {
                let args = tool_call
                    .arguments
                    .as_deref()
                    .and_then(|value| serde_json::from_str::<Value>(value).ok())
                    .unwrap_or_else(|| json!({}));
                parts.push(json!({
                    "functionCall": {
                        "id": tool_call.id,
                        "name": tool_call.name,
                        "args": args,
                    }
                }));
            }
            json!({
                "role": "model",
                "parts": parts,
            })
        }
        MessageRole::System => json!({
            "role": "user",
            "parts": [{"text": message.text_content()}],
        }),
        MessageRole::User => json!({
            "role": "user",
            "parts": if message.content.is_empty() {
                vec![json!({"text": ""})]
            } else {
                message
                    .content
                    .iter()
                    .map(|part| match part {
                        ContentPart::Text { text } => json!({"text": text}),
                        ContentPart::Json { value } => json!({"text": value.to_string()}),
                        ContentPart::ImageUrl { image_url, .. } => json!({
                            "fileData": {
                                "fileUri": image_url,
                                "mimeType": "image/png",
                            }
                        }),
                        ContentPart::Raw { value } => value.clone(),
                    })
                    .collect::<Vec<_>>()
            },
        }),
    }
}

pub(super) fn build_gemini_setup_body(setup: &Value, model: &str) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), Value::String(model.to_string()));
    body.insert("contents".to_string(), Value::Array(Vec::new()));
    body.insert(
        "tools".to_string(),
        clone_present_value(setup.get("tools")).unwrap_or_else(|| json!([])),
    );
    insert_present_value(
        &mut body,
        "system_instruction",
        clone_present_value(setup.get("systemInstruction")),
    );
    insert_present_value(
        &mut body,
        "toolConfig",
        clone_present_value(setup.get("toolConfig")).or_else(|| {
            clone_present_value(
                setup
                    .get("generationConfig")
                    .and_then(|value| value.get("toolConfig")),
            )
        }),
    );
    insert_present_value(
        &mut body,
        "thinkingConfig",
        clone_present_value(
            setup
                .get("generationConfig")
                .and_then(|value| value.get("thinkingConfig")),
        ),
    );
    Value::Object(body)
}

pub(super) fn build_gemini_live_turn_body(session: &GeminiLiveSession) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), Value::String(session.model.clone()));
    body.insert(
        "contents".to_string(),
        Value::Array(
            session
                .messages
                .iter()
                .filter(|message| message.role != MessageRole::System)
                .map(pack_gemini_live_message)
                .collect::<Vec<_>>(),
        ),
    );
    if !session.tools.is_empty() {
        body.insert(
            "tools".to_string(),
            json!([{
                "functionDeclarations": session.tools.iter().map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                })).collect::<Vec<_>>()
            }]),
        );
    }
    insert_present_value(
        &mut body,
        "toolConfig",
        session
            .tool_choice
            .as_ref()
            .map(|tool_choice| json!({"functionCallingConfig": tool_choice})),
    );
    insert_present_value(&mut body, "thinkingConfig", session.reasoning.clone());
    insert_present_value(
        &mut body,
        "system_instruction",
        session.system_instruction.clone(),
    );
    Value::Object(body)
}

pub(super) fn clone_present_value(value: Option<&Value>) -> Option<Value> {
    value.filter(|value| !value.is_null()).cloned()
}

fn insert_present_value(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<Value>,
) {
    if let Some(value) = value.filter(|value| !value.is_null()) {
        object.insert(key.to_string(), value);
    }
}
