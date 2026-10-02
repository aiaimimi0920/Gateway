use std::collections::{HashMap, VecDeque};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage};
use crate::protocol::sse_parse::{format_sse_event, parse_sse_line, SseFrame, SseParseState};
use crate::protocol::upstream_body::collect_bounded_upstream_body;

use super::{
    map_gemini_finish_reason, map_gemini_finish_reason_to_canonical, parse_gemini_tool_call,
};

pub async fn accumulate_gemini_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = collect_bounded_upstream_body(response, "Gemini streaming body").await?;
    accumulate_gemini_stream_buffer(body, model)
}

#[cfg(test)]
pub(crate) fn accumulate_gemini_stream_bytes(
    body: &[u8],
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    accumulate_gemini_stream_buffer(body.to_vec(), model)
}

fn accumulate_gemini_stream_buffer(
    mut buffer: Vec<u8>,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut parser = SseParseState::new();
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut total_tokens = 0u64;
    let mut reported_model = model.to_string();
    let mut finish_reason = None;

    while let Some(pos) = buffer.iter().position(|byte| *byte == b'\n') {
        let mut line = buffer.drain(..=pos).collect::<Vec<_>>();
        if matches!(line.last(), Some(b'\n')) {
            line.pop();
        }
        if matches!(line.last(), Some(b'\r')) {
            line.pop();
        }
        let Ok(line) = std::str::from_utf8(&line) else {
            continue;
        };
        let Some(frame) = parse_sse_line(line, &mut parser) else {
            continue;
        };
        if frame.data == "[DONE]" {
            continue;
        }
        let Ok(raw) = serde_json::from_str::<Value>(&frame.data) else {
            continue;
        };
        if let Some(value) = raw
            .get("modelVersion")
            .or_else(|| raw.get("model"))
            .and_then(|value| value.as_str())
        {
            if !value.trim().is_empty() {
                reported_model = value.to_string();
            }
        }
        if let Some(usage) = raw.get("usageMetadata") {
            prompt_tokens = usage
                .get("promptTokenCount")
                .and_then(|value| value.as_u64())
                .unwrap_or(prompt_tokens);
            completion_tokens = usage
                .get("candidatesTokenCount")
                .and_then(|value| value.as_u64())
                .unwrap_or(completion_tokens);
            total_tokens = usage
                .get("totalTokenCount")
                .and_then(|value| value.as_u64())
                .unwrap_or(total_tokens);
        }
        let candidates = raw
            .get("candidates")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        for candidate in candidates {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|value| value.get("parts"))
                .and_then(|value| value.as_array())
            {
                for part in parts {
                    if let Some(value) = part.get("text").and_then(|value| value.as_str()) {
                        text.push_str(value);
                    }
                    if let Some(function_call) = part
                        .get("functionCall")
                        .or_else(|| part.get("function_call"))
                    {
                        let tool_call = parse_gemini_tool_call(function_call);
                        if !tool_calls.iter().any(|existing: &CanonicalToolCall| {
                            existing.id == tool_call.id
                                && existing.name == tool_call.name
                                && existing.arguments == tool_call.arguments
                        }) {
                            tool_calls.push(tool_call);
                        }
                    }
                }
            }
            if let Some(reason) = candidate
                .get("finishReason")
                .and_then(|value| value.as_str())
            {
                finish_reason = Some(map_gemini_finish_reason_to_canonical(
                    reason,
                    !tool_calls.is_empty(),
                ));
            }
        }
    }

    let usage = if total_tokens > 0 {
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        })
    } else {
        None
    };
    let resolved_finish_reason = finish_reason.unwrap_or_else(|| {
        if tool_calls.is_empty() {
            "stop".to_string()
        } else {
            "tool_calls".to_string()
        }
    });

    Ok(CanonicalRelayResponse {
        model: reported_model,
        text,
        usage,
        tool_calls,
        upstream_status: Some(200),
        finish_reason: Some(resolved_finish_reason),
    })
}

pub fn translate_openai_sse_to_gemini_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_openai_sse_to_gemini_stream_with_error(inner, model)
}

/// 只投影 wire，不构造本地协议错误；保留上游错误类型和对象。
pub fn translate_openai_sse_to_gemini_stream_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    futures::stream::unfold(
        Some((
            Box::pin(inner) as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
            GeminiStreamState::new(model),
        )),
        |state| async move {
            let (mut inner, mut state) = state?;
            loop {
                if let Some(output) = state.outputs.pop_front() {
                    return Some((Ok(Bytes::from(output)), Some((inner, state))));
                }

                let next_chunk = inner.next().await;
                match next_chunk {
                    Some(Ok(bytes)) => {
                        state.buffer.extend_from_slice(&bytes);
                        state.process_buffer();
                    }
                    // 错误是终态；交付错误前释放上游和缓冲，后续不可恢复或伪造完成。
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => return None,
                }
            }
        },
    )
    .fuse()
}

#[cfg(test)]
#[path = "stream_error_tests.rs"]
mod stream_error_tests;

struct PendingGeminiToolCall {
    id: String,
    name: String,
    arguments: String,
}

struct GeminiStreamState {
    buffer: Vec<u8>,
    parser: SseParseState,
    outputs: VecDeque<Vec<u8>>,
    pending_tools: HashMap<usize, PendingGeminiToolCall>,
    latest_usage: Option<TokenUsage>,
}

impl GeminiStreamState {
    fn new(model: String) -> Self {
        let _ = model;
        Self {
            buffer: Vec::new(),
            parser: SseParseState::new(),
            outputs: VecDeque::new(),
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

    fn handle_frame(&mut self, frame: SseFrame) {
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
            self.emit_candidate(json!({
                "candidates": [{
                    "index": 0,
                    "content": {
                        "role": "model",
                        "parts": [{"text": text}],
                    }
                }]
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
            let mut payload = json!({
                "candidates": [{
                    "index": 0,
                    "finishReason": map_gemini_finish_reason(Some(reason), &[]),
                }]
            });
            if let Some(usage) = &self.latest_usage {
                payload["usageMetadata"] = json!({
                    "promptTokenCount": usage.prompt_tokens,
                    "candidatesTokenCount": usage.completion_tokens,
                    "totalTokenCount": usage.total_tokens,
                });
            }
            self.emit_candidate(payload);
        }
    }

    fn handle_tool_call_delta(&mut self, tool_call: &Value, fallback_index: usize) {
        let index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(fallback_index);
        let payload = {
            let entry = self
                .pending_tools
                .entry(index)
                .or_insert_with(|| PendingGeminiToolCall {
                    id: tool_call
                        .get("id")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: String::new(),
                    arguments: String::new(),
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
            if let Some(arguments) = tool_call
                .get("function")
                .and_then(|value| value.get("arguments"))
                .and_then(|value| value.as_str())
            {
                entry.arguments.push_str(arguments);
            }

            if entry.name.is_empty() {
                None
            } else {
                let args =
                    serde_json::from_str::<Value>(&entry.arguments).unwrap_or_else(|_| json!({}));
                Some(json!({
                    "candidates": [{
                        "index": 0,
                        "content": {
                            "role": "model",
                            "parts": [{
                                "functionCall": {
                                    "id": entry.id,
                                    "name": entry.name,
                                    "args": args,
                                }
                            }]
                        }
                    }]
                }))
            }
        };

        let Some(payload) = payload else {
            return;
        };
        self.emit_candidate(payload);
    }

    fn emit_candidate(&mut self, payload: Value) {
        self.outputs
            .push_back(format_sse_event(None, &payload.to_string()).into_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulate_gemini_stream_bytes_reads_tool_calls_and_usage() {
        let body = br#"data: {"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"id":"call_weather","name":"weather","args":{"city":"Hangzhou"}}}]},"finishReason":"STOP"}],"modelVersion":"gemini-tool-fixture","usageMetadata":{"promptTokenCount":4,"candidatesTokenCount":2,"totalTokenCount":6}}

data: [DONE]

"#;

        let response = accumulate_gemini_stream_bytes(body, "gemini-2.5-pro").unwrap();
        assert_eq!(response.model, "gemini-tool-fixture");
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            response.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
        assert_eq!(response.usage.unwrap().total_tokens, 6);
    }

    #[test]
    fn accumulate_gemini_stream_bytes_reads_text_deltas() {
        let body = br#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"hello "}]} }],"modelVersion":"gemini-text-fixture"}

data: {"candidates":[{"content":{"role":"model","parts":[{"text":"world"}]},"finishReason":"STOP"}]}

data: [DONE]

"#;

        let response = accumulate_gemini_stream_bytes(body, "gemini-2.5-pro").unwrap();
        assert_eq!(response.model, "gemini-text-fixture");
        assert_eq!(response.text, "hello world");
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
        assert!(response.tool_calls.is_empty());
    }
}
