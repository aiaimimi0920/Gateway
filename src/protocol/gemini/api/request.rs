use serde_json::{json, Value};

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    MessageRole,
};
use crate::protocol::tool_choice;

pub fn pack_request(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = json!({
        "model": model,
        "contents": build_contents(req),
    });

    if let Some(system) = req.system_message().filter(|value| !value.is_empty()) {
        body["system_instruction"] = json!({
            "parts": [{"text": system}]
        });
    }

    if !req.tools.is_empty() {
        body["tools"] = json!([{
            "functionDeclarations": req.tools.iter().map(pack_tool).collect::<Vec<_>>()
        }]);
    }

    if let Some(tool_choice) = pack_tool_choice(req.tool_choice.as_ref()) {
        body["toolConfig"] = json!({
            "functionCallingConfig": tool_choice
        });
    }

    if let Some(reasoning) = &req.reasoning {
        body["thinkingConfig"] = reasoning.clone();
    }

    if stream {
        body["stream"] = json!(true);
    }

    for (key, value) in &req.extra {
        if try_pack_generation_config_field(&mut body, key, value) {
            continue;
        }
        body[key] = value.clone();
    }

    body
}

fn try_pack_generation_config_field(body: &mut Value, key: &str, value: &Value) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    let Some(config) = body
        .as_object_mut()
        .map(|map| map.entry("generationConfig").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut)
    else {
        return false;
    };

    match normalized.as_str() {
        "temperature" => {
            config.insert("temperature".to_string(), value.clone());
            true
        }
        "top_p" | "topp" => {
            config.insert("topP".to_string(), value.clone());
            true
        }
        "top_k" | "topk" => {
            config.insert("topK".to_string(), value.clone());
            true
        }
        "max_tokens" | "max_completion_tokens" | "max_output_tokens" | "maxoutputtokens" => {
            config.insert("maxOutputTokens".to_string(), value.clone());
            true
        }
        "stop" | "stop_sequences" | "stopsequences" => {
            let stop_sequences = match value {
                Value::String(text) => json!([text]),
                Value::Array(items) => Value::Array(items.clone()),
                _ => value.clone(),
            };
            config.insert("stopSequences".to_string(), stop_sequences);
            true
        }
        "n" | "candidate_count" | "candidatecount" => {
            config.insert("candidateCount".to_string(), value.clone());
            true
        }
        _ => false,
    }
}

fn build_contents(req: &CanonicalRelayRequest) -> Vec<Value> {
    let mut contents = Vec::new();
    for (index, msg) in req.messages.iter().enumerate() {
        if msg.role == MessageRole::System {
            continue;
        }
        if let Some(value) = pack_message(req, index, msg) {
            contents.push(value);
        }
    }
    contents
}

fn pack_message(
    req: &CanonicalRelayRequest,
    index: usize,
    msg: &CanonicalMessage,
) -> Option<Value> {
    match msg.role {
        MessageRole::System => None,
        MessageRole::User => Some(json!({
            "role": "user",
            "parts": pack_parts(&msg.content),
        })),
        MessageRole::Assistant => {
            let mut parts = pack_parts(&msg.content);
            for tool_call in &msg.tool_calls {
                parts.push(pack_tool_call(tool_call));
            }
            Some(json!({
                "role": "model",
                "parts": parts,
            }))
        }
        MessageRole::Tool => {
            let tool_name = msg
                .name
                .clone()
                .or_else(|| find_tool_name(&req.messages[..index], msg.tool_call_id.as_deref()));
            Some(json!({
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "name": tool_name.unwrap_or_else(|| "unknown".to_string()),
                        "response": pack_tool_result_payload(msg),
                    }
                }]
            }))
        }
    }
}

fn pack_parts(parts: &[ContentPart]) -> Vec<Value> {
    if parts.is_empty() {
        return vec![json!({"text": ""})];
    }

    parts
        .iter()
        .map(|part| match part {
            ContentPart::Text { text } => json!({"text": text}),
            ContentPart::ImageUrl { image_url, .. } => json!({
                "fileData": {
                    "fileUri": image_url,
                    "mimeType": guess_image_mime(image_url),
                }
            }),
            ContentPart::Json { value } => json!({"text": value.to_string()}),
            ContentPart::Raw { value } => value.clone(),
        })
        .collect()
}

fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "parameters": tool
            .input_schema
            .clone()
            .unwrap_or_else(|| json!({"type":"object","properties":{}})),
    })
}

fn pack_tool_call(tool_call: &CanonicalToolCall) -> Value {
    json!({
        "functionCall": {
            "id": tool_call.id,
            "name": tool_call.name,
            "args": tool_call
                .arguments
                .as_deref()
                .and_then(|value| serde_json::from_str::<Value>(value).ok())
                .unwrap_or_else(|| json!({})),
        }
    })
}

fn pack_tool_result_payload(msg: &CanonicalMessage) -> Value {
    if msg.content.len() == 1 {
        if let ContentPart::Json { value } = &msg.content[0] {
            return value.clone();
        }
    }
    if msg.content.len() == 1 {
        if let Some(text) = msg.content[0].as_text() {
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                return value;
            }
            return json!({ "content": text });
        }
    }
    json!({
        "content": msg.text_content(),
    })
}

fn pack_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    tool_choice::pack_gemini_tool_choice(tool_choice)
}

fn find_tool_name(history: &[CanonicalMessage], tool_call_id: Option<&str>) -> Option<String> {
    let tool_call_id = tool_call_id?;
    history.iter().rev().find_map(|message| {
        message.tool_calls.iter().find_map(|tool_call| {
            if tool_call.id.as_deref() == Some(tool_call_id) {
                tool_call.name.clone()
            } else {
                None
            }
        })
    })
}

fn guess_image_mime(image_url: &str) -> &'static str {
    let lower = image_url.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalMessage, EndpointKind, ProtocolFamily};
    use std::collections::HashMap;

    fn base_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gemini-2.5-pro".to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "Be helpful.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "hi".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn packs_tools_and_tool_choice() {
        let mut req = base_request();
        req.tools.push(CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: Some("Get weather".to_string()),
            input_schema: Some(json!({"type":"object"})),
            raw: HashMap::new(),
        });
        req.tool_choice = Some(json!("required"));
        let body = pack_request(&req, "gemini-2.5-pro", false);
        assert_eq!(
            body["tools"][0]["functionDeclarations"][0]["name"],
            "weather"
        );
        assert_eq!(body["toolConfig"]["functionCallingConfig"]["mode"], "ANY");
    }

    #[test]
    fn packs_tool_result_using_function_response() {
        let mut req = base_request();
        req.messages.push(CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![CanonicalToolCall {
                id: Some("call_1".to_string()),
                call_type: "function".to_string(),
                name: Some("weather".to_string()),
                arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                raw: HashMap::new(),
            }],
        });
        req.messages.push(CanonicalMessage {
            role: MessageRole::Tool,
            content: vec![ContentPart::Text {
                text: "{\"ok\":true}".to_string(),
            }],
            name: None,
            tool_call_id: Some("call_1".to_string()),
            tool_calls: vec![],
        });
        let body = pack_request(&req, "gemini-2.5-pro", false);
        assert_eq!(
            body["contents"][2]["parts"][0]["functionResponse"]["name"],
            "weather"
        );
    }

    #[test]
    fn packs_common_generation_fields_into_generation_config() {
        let mut req = base_request();
        req.extra.insert("temperature".to_string(), json!(0));
        req.extra.insert("top_p".to_string(), json!(0.5));
        req.extra.insert("max_tokens".to_string(), json!(128));
        req.extra
            .insert("stop".to_string(), json!(["done", "halt"]));
        let body = pack_request(&req, "gemini-2.5-pro", false);
        assert_eq!(body["generationConfig"]["temperature"], json!(0));
        assert_eq!(body["generationConfig"]["topP"], json!(0.5));
        assert_eq!(body["generationConfig"]["maxOutputTokens"], json!(128));
        assert_eq!(
            body["generationConfig"]["stopSequences"],
            json!(["done", "halt"])
        );
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("stop").is_none());
    }
}
