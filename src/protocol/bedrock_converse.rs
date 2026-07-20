use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};
use futures::{Stream, StreamExt};
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::accio::normalize_tool_args;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    EndpointKind, MessageRole, ProtocolFamily, TokenUsage,
};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};
use crate::protocol::tool_choice;

pub fn pack_bedrock_converse(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = json!({
        "modelId": model,
        "messages": build_messages(req),
    });

    if let Some(system) = req.system_message().filter(|value| !value.is_empty()) {
        body["system"] = json!([{ "text": system }]);
    }

    if !req.tools.is_empty() || req.tool_choice.is_some() {
        let mut tool_config = json!({});
        if !req.tools.is_empty() {
            tool_config["tools"] = json!(req.tools.iter().map(pack_tool).collect::<Vec<_>>());
        }
        if let Some(tool_choice) = pack_tool_choice(req.tool_choice.as_ref()) {
            tool_config["toolChoice"] = tool_choice;
        }
        body["toolConfig"] = tool_config;
    }

    if stream {
        body["stream"] = json!(true);
    }

    for (key, value) in &req.extra {
        body[key] = value.clone();
    }

    body
}

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

pub fn build_converse_success(
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
    for tool_call in tool_calls {
        let input = tool_call
            .arguments
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .unwrap_or_else(|| json!({}));
        content.push(json!({
            "toolUse": {
                "toolUseId": tool_call.id,
                "name": tool_call.name,
                "input": input,
            }
        }));
    }

    let mut body = json!({
        "output": {
            "message": {
                "role": "assistant",
                "content": content,
            }
        },
        "stopReason": map_bedrock_finish_reason(finish_reason, tool_calls),
        "model": model,
    });

    if let Some(usage) = usage {
        body["usage"] = json!({
            "inputTokens": usage.prompt_tokens,
            "outputTokens": usage.completion_tokens,
            "totalTokens": usage.total_tokens,
        });
    }

    body
}

pub fn translate_openai_sse_to_bedrock_eventstream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            BedrockStreamState::new(model),
        ),
        |(mut inner, mut state)| async move {
            loop {
                if let Some(output) = state.outputs.pop_front() {
                    return Some((Ok(output), (inner, state)));
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

fn build_messages(req: &CanonicalRelayRequest) -> Vec<Value> {
    let mut messages = Vec::new();
    for (index, msg) in req.messages.iter().enumerate() {
        if msg.role == MessageRole::System {
            continue;
        }
        if let Some(value) = pack_message(req, index, msg) {
            messages.push(value);
        }
    }
    messages
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
            "content": pack_content_parts(&msg.content),
        })),
        MessageRole::Assistant => {
            let mut content = pack_content_parts(&msg.content);
            for tool_call in &msg.tool_calls {
                content.push(json!({
                    "toolUse": {
                        "toolUseId": tool_call.id,
                        "name": tool_call.name,
                        "input": tool_call
                            .arguments
                            .as_deref()
                            .and_then(|value| serde_json::from_str::<Value>(value).ok())
                            .unwrap_or_else(|| json!({})),
                    }
                }));
            }
            Some(json!({
                "role": "assistant",
                "content": content,
            }))
        }
        MessageRole::Tool => {
            let tool_name = msg
                .name
                .clone()
                .or_else(|| find_tool_name(&req.messages[..index], msg.tool_call_id.as_deref()));
            let result_value = if msg.content.len() == 1 {
                match &msg.content[0] {
                    ContentPart::Json { value } => value.clone(),
                    _ => serde_json::from_str::<Value>(&msg.text_content())
                        .unwrap_or_else(|_| json!({ "content": msg.text_content() })),
                }
            } else {
                json!({ "content": msg.text_content() })
            };

            Some(json!({
                "role": "user",
                "content": [{
                    "toolResult": {
                        "toolUseId": msg.tool_call_id,
                        "status": "success",
                        "content": [{
                            "json": result_value
                        }],
                        "name": tool_name,
                    }
                }]
            }))
        }
    }
}

fn pack_content_parts(parts: &[ContentPart]) -> Vec<Value> {
    if parts.is_empty() {
        return vec![json!({"text": ""})];
    }

    parts
        .iter()
        .map(|part| match part {
            ContentPart::Text { text } => json!({"text": text}),
            ContentPart::Json { value } => json!({"json": value}),
            ContentPart::ImageUrl { image_url, .. } => json!({
                "image": {
                    "format": guess_image_format(image_url),
                    "source": { "url": image_url }
                }
            }),
            ContentPart::Raw { value } => value.clone(),
        })
        .collect()
}

fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "toolSpec": {
            "name": tool.name,
            "description": tool.description,
            "inputSchema": {
                "json": tool
                    .input_schema
                    .clone()
                    .unwrap_or_else(|| json!({"type":"object","properties":{}}))
            }
        }
    })
}

fn pack_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    tool_choice::pack_bedrock_tool_choice(tool_choice)
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

fn guess_image_format(image_url: &str) -> &'static str {
    let lower = image_url.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "jpeg"
    } else if lower.ends_with(".webp") {
        "webp"
    } else if lower.ends_with(".gif") {
        "gif"
    } else {
        "png"
    }
}

pub fn default_path(model: &str, stream: bool) -> String {
    if stream {
        format!("/model/{model}/converse-stream")
    } else {
        format!("/model/{model}/converse")
    }
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

fn map_bedrock_finish_reason(
    finish_reason: Option<&str>,
    tool_calls: &[CanonicalToolCall],
) -> &'static str {
    match finish_reason.unwrap_or(if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    }) {
        "tool_calls" => "tool_use",
        "length" => "max_tokens",
        "content_filter" => "guardrail_intervened",
        _ => "end_turn",
    }
}

struct PendingBedrockToolCall {
    id: String,
    name: String,
}

struct BedrockStreamState {
    model: String,
    buffer: Vec<u8>,
    parser: SseParseState,
    outputs: VecDeque<Bytes>,
    started: bool,
    text_started: bool,
    closed_tool_blocks: BTreeSet<usize>,
    pending_tools: HashMap<usize, PendingBedrockToolCall>,
    latest_usage: Option<TokenUsage>,
}

impl BedrockStreamState {
    fn new(model: String) -> Self {
        Self {
            model,
            buffer: Vec::new(),
            parser: SseParseState::new(),
            outputs: VecDeque::new(),
            started: false,
            text_started: false,
            closed_tool_blocks: BTreeSet::new(),
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
            self.ensure_message_started();
            if !self.text_started {
                self.text_started = true;
            }
            self.emit_event(json!({
                "contentBlockDelta": {
                    "contentBlockIndex": 0,
                    "delta": { "text": text }
                }
            }));
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
            self.emit_event(json!({
                "messageStop": {
                    "stopReason": map_bedrock_finish_reason(Some(reason), &[])
                }
            }));
            if let Some(usage) = &self.latest_usage {
                self.emit_event(json!({
                    "metadata": {
                        "usage": {
                            "inputTokens": usage.prompt_tokens,
                            "outputTokens": usage.completion_tokens,
                            "totalTokens": usage.total_tokens
                        }
                    }
                }));
            }
        }
    }

    fn handle_tool_call_delta(&mut self, tool_call: &Value, fallback_index: usize) {
        let index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(fallback_index);
        self.ensure_message_started();
        let mut start_payload = None;
        let mut delta_payload = None;

        {
            let entry = self
                .pending_tools
                .entry(index)
                .or_insert_with(|| PendingBedrockToolCall {
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
                }
            }

            if !self.closed_tool_blocks.contains(&index) && !entry.name.is_empty() {
                start_payload = Some(json!({
                    "contentBlockStart": {
                        "contentBlockIndex": index as i64,
                        "start": {
                            "toolUse": {
                                "toolUseId": entry.id,
                                "name": entry.name
                            }
                        }
                    }
                }));
            }

            if let Some(arguments) = tool_call
                .get("function")
                .and_then(|value| value.get("arguments"))
                .and_then(|value| value.as_str())
            {
                delta_payload = Some(json!({
                    "contentBlockDelta": {
                        "contentBlockIndex": index as i64,
                        "delta": {
                            "toolUse": {
                                "input": normalize_tool_args(arguments)
                            }
                        }
                    }
                }));
            }
        }

        if let Some(payload) = start_payload {
            self.emit_event(payload);
            self.closed_tool_blocks.insert(index);
        }
        if let Some(payload) = delta_payload {
            self.emit_event(payload);
        }
    }

    fn ensure_message_started(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.emit_event(json!({
            "messageStart": {
                "model": self.model
            }
        }));
    }

    fn emit_event(&mut self, payload: Value) {
        self.outputs
            .push_back(encode_aws_eventstream_json(&payload));
    }
}

fn encode_aws_eventstream_json(payload: &Value) -> Bytes {
    static CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

    let payload = serde_json::to_vec(payload).unwrap_or_default();
    let headers_len = 0u32;
    let total_len = 16u32 + payload.len() as u32;

    let mut frame = Vec::with_capacity(total_len as usize);
    frame.extend_from_slice(&total_len.to_be_bytes());
    frame.extend_from_slice(&headers_len.to_be_bytes());

    let prelude_crc = CRC32.checksum(&frame);
    frame.extend_from_slice(&prelude_crc.to_be_bytes());
    frame.extend_from_slice(&payload);

    let message_crc = CRC32.checksum(&frame);
    frame.extend_from_slice(&message_crc.to_be_bytes());
    Bytes::from(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::CanonicalToolCall;
    use std::collections::HashMap;

    fn base_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
            requested_model: Some("anthropic.claude-3-5-sonnet".to_string()),
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
    fn packs_tool_specs_and_choice() {
        let mut req = base_request();
        req.tools.push(CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: Some("Get weather".to_string()),
            input_schema: Some(json!({"type":"object"})),
            raw: HashMap::new(),
        });
        req.tool_choice = Some(json!("required"));
        let body = pack_bedrock_converse(&req, "anthropic.claude-3-5-sonnet", false);
        assert_eq!(
            body["toolConfig"]["tools"][0]["toolSpec"]["name"],
            "weather"
        );
        assert!(body["toolConfig"]["toolChoice"]["any"].is_object());
    }

    #[test]
    fn packs_tool_result_as_user_tool_result_block() {
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
        let body = pack_bedrock_converse(&req, "anthropic.claude-3-5-sonnet", false);
        assert_eq!(
            body["messages"][2]["content"][0]["toolResult"]["toolUseId"],
            "call_1"
        );
    }

    #[test]
    fn maps_content_filter_to_guardrail_intervened() {
        assert_eq!(
            map_bedrock_finish_reason(Some("content_filter"), &[]),
            "guardrail_intervened"
        );
    }
}
