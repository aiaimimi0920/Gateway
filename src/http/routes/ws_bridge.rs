use std::collections::HashMap;

use serde_json::Value;

use crate::protocol::canonical::{CanonicalToolCall, TokenUsage};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

#[derive(Debug, Clone)]
pub struct OpenAiToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments_delta: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenAiChatChunk {
    pub content_delta: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
    pub tool_call_deltas: Vec<OpenAiToolCallDelta>,
    pub done: bool,
}

#[derive(Debug, Default)]
pub struct OpenAiSseAccumulator {
    buffer: Vec<u8>,
    parser: SseParseState,
}

impl OpenAiSseAccumulator {
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Vec<OpenAiChatChunk> {
        self.buffer.extend_from_slice(bytes);
        let mut chunks = Vec::new();
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
                if let Some(chunk) = parse_openai_chat_frame(&frame.data) {
                    chunks.push(chunk);
                }
            }
        }
        chunks
    }
}

#[derive(Debug, Clone)]
pub struct PendingToolCall {
    pub id: String,
    pub item_id: String,
    pub name: String,
    pub arguments: String,
}

impl PendingToolCall {
    pub fn new(item_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            item_id,
            name: String::new(),
            arguments: String::new(),
        }
    }

    pub fn apply_delta(&mut self, delta: &OpenAiToolCallDelta) -> Option<String> {
        if let Some(id) = delta.id.as_deref().filter(|value| !value.trim().is_empty()) {
            self.id = id.to_string();
        }
        if let Some(name) = delta
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            self.name = name.to_string();
        }
        if let Some(arguments) = delta.arguments_delta.as_deref() {
            self.arguments.push_str(arguments);
            return Some(arguments.to_string());
        }
        None
    }

    pub fn as_tool_call(&self) -> CanonicalToolCall {
        CanonicalToolCall {
            id: Some(self.id.clone()),
            call_type: "function".to_string(),
            name: if self.name.is_empty() {
                None
            } else {
                Some(self.name.clone())
            },
            arguments: if self.arguments.is_empty() {
                None
            } else {
                Some(self.arguments.clone())
            },
            raw: HashMap::new(),
        }
    }

    pub fn arguments_json(&self) -> Value {
        serde_json::from_str(&self.arguments).unwrap_or_else(|_| Value::Object(Default::default()))
    }
}

fn parse_openai_chat_frame(data: &str) -> Option<OpenAiChatChunk> {
    if data == "[DONE]" {
        return Some(OpenAiChatChunk {
            content_delta: None,
            finish_reason: None,
            usage: None,
            tool_call_deltas: Vec::new(),
            done: true,
        });
    }

    let raw = serde_json::from_str::<Value>(data).ok()?;
    let choice = raw
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|value| value.first());

    let usage = raw.get("usage").map(|usage| TokenUsage {
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
        cache_creation_input_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|value| value.as_u64()),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|value| value.as_u64()),
    });

    let tool_call_deltas = choice
        .and_then(|value| value.get("delta"))
        .and_then(|value| value.get("tool_calls"))
        .and_then(|value| value.as_array())
        .map(|tool_calls| {
            tool_calls
                .iter()
                .enumerate()
                .map(|(fallback_index, tool_call)| OpenAiToolCallDelta {
                    index: tool_call
                        .get("index")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as usize)
                        .unwrap_or(fallback_index),
                    id: tool_call
                        .get("id")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    name: tool_call
                        .get("function")
                        .and_then(|value| value.get("name"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    arguments_delta: tool_call
                        .get("function")
                        .and_then(|value| value.get("arguments"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(OpenAiChatChunk {
        content_delta: choice
            .and_then(|value| value.get("delta"))
            .and_then(|value| value.get("content"))
            .and_then(|value| value.as_str())
            .map(str::to_string),
        finish_reason: choice
            .and_then(|value| value.get("finish_reason"))
            .and_then(|value| value.as_str())
            .map(str::to_string),
        usage,
        tool_call_deltas,
        done: false,
    })
}
