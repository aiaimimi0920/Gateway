use std::collections::{HashMap, VecDeque};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    EndpointKind, MessageRole, ProtocolFamily, TokenUsage,
};
use crate::protocol::sse_parse::{format_sse_event, parse_sse_line, SseParseState};
use crate::protocol::tool_choice;

pub fn pack_cohere(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = json!({
        "model": model,
        "stream": stream,
        "messages": req
            .messages
            .iter()
            .map(pack_message)
            .collect::<Vec<_>>(),
    });

    if !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(pack_tool).collect::<Vec<_>>());
    }

    if let Some(tool_choice) = pack_tool_choice(req.tool_choice.as_ref()) {
        body["tool_choice"] = tool_choice;
    }

    if let Some(reasoning) = &req.reasoning {
        body["thinking"] = reasoning.clone();
    }

    for (key, value) in &req.extra {
        body[key] = value.clone();
    }

    body
}

pub fn normalize_chat_v2(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let stream = body
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let raw_messages = body
        .get("messages")
        .and_then(|value| value.as_array())
        .ok_or_else(|| GatewayError::bad_request("missing or invalid `messages` array"))?;

    let mut messages = Vec::new();
    for raw_message in raw_messages {
        messages.push(normalize_cohere_message(raw_message));
    }

    let tools = parse_cohere_tools(body.get("tools"));
    let tool_choice = tool_choice::canonicalize_tool_choice(body.get("tool_choice"));

    const KNOWN_FIELDS: &[&str] = &["model", "stream", "messages", "tools", "tool_choice"];
    let mut extra = HashMap::new();
    if let Value::Object(map) = &body {
        for (key, value) in map {
            if !KNOWN_FIELDS.contains(&key.as_str()) {
                extra.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::CohereChat,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: model,
        stream,
        messages,
        tools,
        tool_choice,
        reasoning: body.get("thinking").cloned(),
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key: None,
        extra,
    })
}

pub fn build_chat_v2_success(
    response_id: &str,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut content = Vec::new();
    if !text.is_empty() || tool_calls.is_empty() {
        content.push(json!({"text": text}));
    }

    let mut body = json!({
        "id": response_id,
        "model": model,
        "message": {
            "role": "assistant",
            "content": content,
        },
        "finish_reason": map_cohere_finish_reason(finish_reason, tool_calls),
    });

    if !tool_calls.is_empty() {
        body["message"]["tool_calls"] =
            json!(tool_calls.iter().map(pack_tool_call).collect::<Vec<_>>());
    }

    if let Some(usage) = usage {
        body["usage"] = json!({
            "input_tokens": usage.prompt_tokens,
            "output_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
        });
    }

    body
}

pub fn translate_openai_sse_to_cohere_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            CohereStreamState::new(model),
        ),
        |(mut inner, mut state)| async move {
            loop {
                if let Some(output) = state.outputs.pop_front() {
                    return Some((Ok(Bytes::from(output)), (inner, state)));
                }

                let next_chunk = inner.next().await;
                match next_chunk {
                    Some(Ok(bytes)) => {
                        state.buffer.extend_from_slice(&bytes);
                        state.process_buffer();
                    }
                    Some(Err(error)) => return Some((Err(error), (inner, state))),
                    None => return None,
                }
            }
        },
    )
}

fn pack_message(message: &CanonicalMessage) -> Value {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    };

    let content = if message.content.len() == 1 {
        if let Some(text) = message.content[0].as_text() {
            json!(text)
        } else {
            json!(message
                .content
                .iter()
                .map(pack_content_part)
                .collect::<Vec<_>>())
        }
    } else if message.content.is_empty() {
        json!("")
    } else {
        json!(message
            .content
            .iter()
            .map(pack_content_part)
            .collect::<Vec<_>>())
    };

    let mut value = json!({
        "role": role,
        "content": content,
    });

    if let Some(tool_call_id) = &message.tool_call_id {
        value["tool_call_id"] = json!(tool_call_id);
    }

    if !message.tool_calls.is_empty() {
        value["tool_calls"] = json!(message
            .tool_calls
            .iter()
            .map(pack_tool_call)
            .collect::<Vec<_>>());
    }

    value
}

fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool
                .input_schema
                .clone()
                .unwrap_or_else(|| json!({"type":"object","properties":{}})),
        }
    })
}

fn pack_tool_call(tool_call: &CanonicalToolCall) -> Value {
    json!({
        "id": tool_call.id,
        "type": "function",
        "function": {
            "name": tool_call.name,
            "arguments": tool_call.arguments,
        }
    })
}

fn pack_content_part(part: &ContentPart) -> Value {
    match part {
        ContentPart::Text { text } => json!({"type": "text", "text": text}),
        ContentPart::Json { value } => json!({"type": "json", "value": value}),
        ContentPart::ImageUrl { image_url, .. } => json!({"type": "image_url", "url": image_url}),
        ContentPart::Raw { value } => value.clone(),
    }
}

fn pack_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    tool_choice::pack_cohere_tool_choice(tool_choice)
}

pub fn default_path(_stream: bool) -> &'static str {
    "/v2/chat"
}

fn normalize_cohere_message(raw_message: &Value) -> CanonicalMessage {
    let role = match raw_message
        .get("role")
        .and_then(|value| value.as_str())
        .unwrap_or("user")
    {
        "system" => MessageRole::System,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        _ => MessageRole::User,
    };

    let content = match raw_message.get("content") {
        Some(Value::String(text)) => vec![ContentPart::Text { text: text.clone() }],
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|part| {
                if let Some(text) = part.get("text").and_then(|value| value.as_str()) {
                    ContentPart::Text {
                        text: text.to_string(),
                    }
                } else {
                    ContentPart::Raw {
                        value: part.clone(),
                    }
                }
            })
            .collect(),
        Some(other) => vec![ContentPart::Raw {
            value: other.clone(),
        }],
        None => vec![],
    };

    CanonicalMessage {
        role,
        content,
        name: raw_message
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        tool_call_id: raw_message
            .get("tool_call_id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        tool_calls: parse_cohere_tool_calls(raw_message.get("tool_calls")),
    }
}

fn parse_cohere_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let Some(items) = raw.and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|tool| {
            let function = tool.get("function");
            CanonicalTool {
                tool_type: "function".to_string(),
                name: function
                    .and_then(|value| value.get("name"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                description: function
                    .and_then(|value| value.get("description"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                input_schema: function.and_then(|value| value.get("parameters")).cloned(),
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

fn parse_cohere_tool_calls(raw: Option<&Value>) -> Vec<CanonicalToolCall> {
    let Some(items) = raw.and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|tool_call| {
            let function = tool_call.get("function");
            CanonicalToolCall {
                id: tool_call
                    .get("id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                call_type: "function".to_string(),
                name: tool_call
                    .get("name")
                    .or_else(|| function.and_then(|value| value.get("name")))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                arguments: tool_call
                    .get("arguments")
                    .or_else(|| function.and_then(|value| value.get("arguments")))
                    .map(|value| {
                        if let Some(text) = value.as_str() {
                            text.to_string()
                        } else {
                            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
                        }
                    }),
                raw: HashMap::new(),
            }
        })
        .collect()
}

fn map_cohere_finish_reason(
    finish_reason: Option<&str>,
    tool_calls: &[CanonicalToolCall],
) -> &'static str {
    match finish_reason.unwrap_or(if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    }) {
        "tool_calls" => "TOOL_CALL",
        "length" => "MAX_TOKENS",
        _ => "COMPLETE",
    }
}

struct PendingCohereToolCall {
    id: String,
    name: String,
}

struct CohereStreamState {
    model: String,
    buffer: Vec<u8>,
    parser: SseParseState,
    outputs: VecDeque<Vec<u8>>,
    started: bool,
    pending_tools: HashMap<usize, PendingCohereToolCall>,
    latest_usage: Option<TokenUsage>,
}

impl CohereStreamState {
    fn new(model: String) -> Self {
        Self {
            model,
            buffer: Vec::new(),
            parser: SseParseState::new(),
            outputs: VecDeque::new(),
            started: false,
            pending_tools: HashMap::new(),
            latest_usage: None,
        }
    }

    fn process_buffer(&mut self) {
        while let Some(pos) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=pos).collect::<Vec<_>>();
            if matches!(line.last(), Some(b'\n')) {
                line.pop();
            }
            if matches!(line.last(), Some(b'\r')) {
                line.pop();
            }
            let Ok(line) = std::str::from_utf8(&line) else {
                continue;
            };
            if let Some(frame) = parse_sse_line(line, &mut self.parser) {
                self.handle_frame(frame);
            }
        }
    }

    fn handle_frame(&mut self, frame: crate::protocol::sse_parse::SseFrame) {
        if frame.data == "[DONE]" {
            return;
        }
        let Ok(raw) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };
        let choice = raw
            .get("choices")
            .and_then(|value| value.as_array())
            .and_then(|value| value.first());

        if let Some(usage) = raw.get("usage") {
            self.latest_usage = Some(TokenUsage {
                prompt_tokens: usage
                    .get("prompt_tokens")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
                completion_tokens: usage
                    .get("completion_tokens")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
                total_tokens: usage
                    .get("total_tokens")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            });
        }

        if let Some(text) = choice
            .and_then(|value| value.get("delta"))
            .and_then(|value| value.get("content"))
            .and_then(|value| value.as_str())
        {
            self.ensure_started();
            self.emit(
                "content-delta",
                json!({
                    "delta": {
                        "message": {
                            "content": {
                                "text": text
                            }
                        }
                    }
                }),
            );
        }

        if let Some(tool_calls) = choice
            .and_then(|value| value.get("delta"))
            .and_then(|value| value.get("tool_calls"))
            .and_then(|value| value.as_array())
        {
            for (fallback_index, tool_call) in tool_calls.iter().enumerate() {
                self.handle_tool_call_delta(tool_call, fallback_index);
            }
        }

        if let Some(reason) = choice
            .and_then(|value| value.get("finish_reason"))
            .and_then(|value| value.as_str())
        {
            let usage = self.latest_usage.as_ref().map(|usage| {
                json!({
                    "input_tokens": usage.prompt_tokens,
                    "output_tokens": usage.completion_tokens,
                    "total_tokens": usage.total_tokens,
                })
            });
            let mut payload = json!({
                "delta": {
                    "finish_reason": map_cohere_finish_reason(Some(reason), &[])
                }
            });
            if let Some(usage) = usage {
                payload["delta"]["usage"] = usage;
            }
            self.emit("message-end", payload);
        }
    }

    fn handle_tool_call_delta(&mut self, tool_call: &Value, fallback_index: usize) {
        self.ensure_started();
        let index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(fallback_index);
        let mut start_payload = None;
        let mut delta_payload = None;

        {
            let entry = self
                .pending_tools
                .entry(index)
                .or_insert_with(|| PendingCohereToolCall {
                    id: tool_call
                        .get("id")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: String::new(),
                });

            if let Some(id) = tool_call.get("id").and_then(|value| value.as_str()) {
                if !id.trim().is_empty() {
                    entry.id = id.to_string();
                }
            }
            if let Some(name) = tool_call
                .get("function")
                .and_then(|value| value.get("name"))
                .and_then(|value| value.as_str())
            {
                if !name.trim().is_empty() {
                    entry.name = name.to_string();
                    start_payload = Some(json!({
                        "index": index,
                        "delta": {
                            "message": {
                                "tool_calls": [{
                                    "id": entry.id,
                                    "function": {
                                        "name": entry.name
                                    }
                                }]
                            }
                        }
                    }));
                }
            }

            if let Some(arguments) = tool_call
                .get("function")
                .and_then(|value| value.get("arguments"))
                .and_then(|value| value.as_str())
            {
                delta_payload = Some(json!({
                    "index": index,
                    "delta": {
                        "message": {
                            "tool_calls": {
                                "function": {
                                    "arguments": arguments
                                }
                            }
                        }
                    }
                }));
            }
        }

        if let Some(payload) = start_payload {
            self.emit("tool-call-start", payload);
        }
        if let Some(payload) = delta_payload {
            self.emit("tool-call-delta", payload);
        }
    }

    fn ensure_started(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.emit(
            "message-start",
            json!({
                "delta": {
                    "message": {
                        "role": "assistant",
                        "model": self.model,
                    }
                }
            }),
        );
    }

    fn emit(&mut self, event: &str, payload: Value) {
        self.outputs
            .push_back(format_sse_event(Some(event), &payload.to_string()).into_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn base_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
            requested_model: Some("command-r".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hi".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
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
    fn packs_tool_choice_required() {
        let mut req = base_request();
        req.tool_choice = Some(json!("required"));
        let body = pack_cohere(&req, "command-r", false);
        assert_eq!(body["tool_choice"], "REQUIRED");
    }

    #[test]
    fn packs_assistant_tool_calls() {
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
        let body = pack_cohere(&req, "command-r", false);
        assert_eq!(
            body["messages"][1]["tool_calls"][0]["function"]["name"],
            "weather"
        );
    }
}
