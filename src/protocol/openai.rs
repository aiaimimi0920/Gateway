// ---------------------------------------------------------------------------
// OpenAI protocol adapter — pack / unpack / builder helpers
//
// Converts between CanonicalRelayRequest/Response and the OpenAI
// chat/completions wire format.
// ---------------------------------------------------------------------------

use std::collections::VecDeque;

use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalRelayResponse, CanonicalTool,
    CanonicalToolCall, ContentPart, EndpointKind, MessageRole, ProtocolFamily, TokenUsage,
};
use crate::protocol::sse_parse::{format_sse_event, parse_sse_line, SseFrame, SseParseState};
use crate::protocol::tool_inject;

// ---------------------------------------------------------------------------
// normalize_chat_completions
// ---------------------------------------------------------------------------

/// Normalize an OpenAI `POST /v1/chat/completions` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_chat_completions(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ── messages ──────────────────────────────────────────────────────────

    let raw_messages = body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GatewayError::bad_request("missing or invalid `messages` array"))?;

    let mut messages: Vec<CanonicalMessage> = Vec::with_capacity(raw_messages.len());

    for raw_msg in raw_messages {
        let role_str = raw_msg
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GatewayError::bad_request("message missing `role` field"))?;

        let role = parse_role(role_str)?;

        let content = parse_openai_content(raw_msg.get("content"))?;

        let name = raw_msg
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let tool_call_id = raw_msg
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let tool_calls = parse_openai_message_tool_calls(raw_msg);

        messages.push(CanonicalMessage {
            role,
            content,
            name,
            tool_call_id,
            tool_calls,
        });
    }

    // ── tools ─────────────────────────────────────────────────────────────

    let tools = parse_openai_tools(body.get("tools"));
    let tool_choice = body.get("tool_choice").cloned();

    // ── extra (passthrough) parameters ────────────────────────────────────

    // Known top-level fields that are handled explicitly above.
    const KNOWN_FIELDS: &[&str] = &[
        "model",
        "messages",
        "stream",
        "tools",
        "tool_choice",
        "user",
        "reasoning",
    ];

    let mut extra = std::collections::HashMap::new();
    // Capture ALL unknown fields into extra so vendor extensions are preserved.
    if let Value::Object(map) = &body {
        for (key, value) in map {
            if !KNOWN_FIELDS.contains(&key.as_str()) {
                extra.insert(key.clone(), value.clone());
            }
        }
    }

    // ── session / user ────────────────────────────────────────────────────

    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: model,
        stream,
        messages,
        tools,
        tool_choice,
        reasoning: body.get("reasoning").cloned(),
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra,
    })
}

/// Normalize an OpenAI `POST /v1/completions` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_legacy_completions(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let prompt = body
        .get("prompt")
        .ok_or_else(|| GatewayError::bad_request("missing `prompt` field"))?;
    let messages = parse_prompt_messages(prompt);
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::Completions,
        requested_model: model,
        stream,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

/// Normalize an OpenAI `POST /v1/embeddings` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_embeddings(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let input = body
        .get("input")
        .ok_or_else(|| GatewayError::bad_request("missing `input` field"))?;
    let messages = parse_prompt_messages(input);
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::Embeddings,
        requested_model: model,
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

/// Normalize an OpenAI `POST /v1/audio/speech` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_audio_speech(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let input = body
        .get("input")
        .ok_or_else(|| GatewayError::bad_request("missing `input` field"))?;
    let messages = parse_prompt_messages(input);
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioSpeech,
        requested_model: model,
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

/// Normalize an OpenAI `POST /v1/audio/transcriptions` request body into a
/// [`CanonicalRelayRequest`].
///
/// The route layer converts multipart form data into a JSON object so the
/// canonical pipeline can still inspect, route, quota-check, and audit the
/// request without carrying raw multipart bytes through the hot path.
pub fn normalize_audio_transcriptions(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let file = body
        .get("file")
        .and_then(|value| value.as_object())
        .ok_or_else(|| GatewayError::bad_request("missing `file` field"))?;
    let file_name = file
        .get("file_name")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let prompt = body
        .get("prompt")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let message_seed = prompt
        .clone()
        .or_else(|| file_name.map(|value| format!("transcribe {}", value)))
        .unwrap_or_else(|| "audio transcription request".to_string());
    let messages = vec![CanonicalMessage {
        role: MessageRole::User,
        content: vec![ContentPart::Text { text: message_seed }],
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }];
    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioTranscriptions,
        requested_model: model,
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

// ---------------------------------------------------------------------------
// pack_openai
// ---------------------------------------------------------------------------

/// Pack a [`CanonicalRelayRequest`] into an OpenAI chat/completions JSON body.
pub fn pack_openai(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let is_reasoning = model.starts_with("o1") || model.starts_with("o3");

    // Build messages array — extract system into first element if present.
    let mut json_messages: Vec<Value> = Vec::new();

    // If there is a system message, output it as the first message in the array.
    if let Some(sys_text) = req.system_message() {
        json_messages.push(json!({
            "role": "system",
            "content": sys_text,
        }));
    }

    for msg in &req.messages {
        if msg.role == MessageRole::System {
            // Already handled above.
            continue;
        }
        json_messages.push(pack_openai_message(msg));
    }

    let mut body = json!({
        "model": model,
        "messages": json_messages,
        "stream": stream,
    });

    // ── tools ──────────────────────────────────────────────────────────────

    if !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(pack_openai_tool).collect::<Vec<_>>());
    }
    if let Some(tc) = pack_openai_tool_choice(req.protocol_family, req.tool_choice.as_ref()) {
        body["tool_choice"] = tc;
    }

    // ── extra parameters ───────────────────────────────────────────────────

    for (k, v) in &req.extra {
        match k.as_str() {
            // max_tokens — routing depends on model family
            "max_tokens" if is_reasoning => {
                // For reasoning models, translate to max_completion_tokens
                body["max_completion_tokens"] = v.clone();
            }
            "max_tokens" => {
                body["max_tokens"] = v.clone();
            }
            // Reasoning models should not receive temperature/top_p.
            "temperature" | "top_p" if is_reasoning => {}
            _ => {
                body[k] = v.clone();
            }
        }
    }

    // If the request explicitly carried max_completion_tokens (from a
    // reasoning model request), always forward it verbatim.
    if let Some(mct) = req.extra.get("max_completion_tokens") {
        body["max_completion_tokens"] = mct.clone();
    }

    if stream {
        ensure_stream_usage_requested(&mut body);
    }

    body
}

// ---------------------------------------------------------------------------
// unpack_openai_response
// ---------------------------------------------------------------------------

/// Unpack an OpenAI response JSON body into a [`CanonicalRelayResponse`].
pub fn unpack_openai_response(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let choices = body
        .get("choices")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GatewayError::server_error("OpenAI response missing `choices`"))?;

    if choices.is_empty() {
        return Err(GatewayError::server_error(
            "OpenAI response `choices` is empty",
        ));
    }

    let first = &choices[0];
    let message = first
        .get("message")
        .ok_or_else(|| GatewayError::server_error("OpenAI response choice missing `message`"))?;

    let mut text = match message.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    };

    let mut finish_reason =
        map_openai_finish_reason(first.get("finish_reason").and_then(|v| v.as_str()));

    let mut tool_calls = parse_openai_message_tool_calls(message);
    if tool_calls.is_empty()
        && (text.contains("<tool_calls>")
            || text.contains("<function_calls>")
            || text.contains("<invoke "))
    {
        let parse_result = tool_inject::parse_tool_calls_from_text(&text);
        if parse_result.had_tool_calls {
            text = parse_result.clean_text;
            tool_calls = parse_result.tool_calls;
            finish_reason = Some("tool_calls".to_string());
        }
    }

    let usage = body.get("usage").map(|u| {
        let prompt_tokens = u
            .get("prompt_tokens")
            .or_else(|| u.get("input_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0);
        let completion_tokens = u
            .get("completion_tokens")
            .or_else(|| u.get("output_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0);
        TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: u
                .get("total_tokens")
                .and_then(|t| t.as_u64())
                .unwrap_or(prompt_tokens + completion_tokens),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: u
                .get("prompt_tokens_details")
                .and_then(|details| details.get("cached_tokens"))
                .and_then(|tokens| tokens.as_u64())
                .or_else(|| {
                    u.get("input_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|tokens| tokens.as_u64())
                }),
        }
    });

    let upstream_status = body
        .get("_upstream_status")
        .and_then(|v| v.as_u64())
        .map(|s| s as u16);

    Ok(CanonicalRelayResponse {
        model,
        text,
        usage,
        tool_calls,
        upstream_status,
        finish_reason,
    })
}

// ---------------------------------------------------------------------------
// Response builders (for the relay to re-emit in OpenAI format)
// ---------------------------------------------------------------------------

/// Build a complete, non-streaming OpenAI chat/completions success response.
pub fn build_chat_completions_success(
    response_id: &str,
    created_at: i64,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut message = json!({
        "role": "assistant",
        "content": text,
    });

    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls
            .iter()
            .map(|tc| json!({
                "id": tc.id,
                "type": tc.call_type,
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments,
                }
            }))
            .collect::<Vec<_>>());
    }

    let mut response = json!({
        "id": response_id,
        "object": "chat.completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": normalize_openai_wire_finish_reason(finish_reason, !tool_calls.is_empty()),
        }],
    });

    if let Some(u) = usage {
        response["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    response
}

/// Build a streaming SSE delta chunk in OpenAI format.
pub fn build_chat_completions_delta(
    response_id: &str,
    created_at: i64,
    model: &str,
    delta_text: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": delta_text,
            },
            "finish_reason": null,
        }],
    })
}

/// Build a streaming stop/final chunk in OpenAI format.
pub fn build_chat_completions_stop(
    response_id: &str,
    created_at: i64,
    model: &str,
    usage: Option<&TokenUsage>,
) -> Value {
    let mut chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": normalize_openai_wire_finish_reason(Some("stop"), false),
        }],
    });

    if let Some(u) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    chunk
}

pub fn build_legacy_completions_success(
    response_id: &str,
    created_at: i64,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    finish_reason: Option<&str>,
) -> Value {
    let mut response = json!({
        "id": response_id,
        "object": "text_completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "text": text,
            "finish_reason": normalize_openai_wire_finish_reason(finish_reason, false),
        }],
    });

    if let Some(u) = usage {
        response["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    response
}

pub fn build_legacy_completions_delta(
    response_id: &str,
    created_at: i64,
    model: &str,
    delta_text: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "text_completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "text": delta_text,
            "finish_reason": null,
        }],
    })
}

pub fn build_legacy_completions_stop(
    response_id: &str,
    created_at: i64,
    model: &str,
    usage: Option<&TokenUsage>,
    finish_reason: Option<&str>,
) -> Value {
    let mut chunk = json!({
        "id": response_id,
        "object": "text_completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "text": "",
            "finish_reason": normalize_openai_wire_finish_reason(finish_reason, false),
        }],
    });

    if let Some(u) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    chunk
}

pub fn translate_openai_chat_sse_to_legacy_completions(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let state = OpenAiChatToLegacyCompletionsState {
        buffer: Vec::new(),
        parser: SseParseState::new(),
        response_id: format!("cmpl_{}", uuid::Uuid::new_v4().as_simple()),
        model,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        outputs: VecDeque::new(),
        native_passthrough: false,
        latest_usage: None,
        pending_finish_reason: None,
        final_emitted: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut st, mut done)| async move {
            use futures::StreamExt;

            if done {
                return None;
            }

            loop {
                if let Some(output) = st.outputs.pop_front() {
                    done = st.final_emitted && st.outputs.is_empty();
                    return Some((Ok(Bytes::from(output)), (stream, st, done)));
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        st.buffer.extend_from_slice(&chunk);
                        st.drain_frames();
                    }
                    Some(Err(error)) => return Some((Err(error), (stream, st, true))),
                    None => {
                        st.drain_frames();
                        st.flush_pending_frame();
                        st.emit_final_if_needed(None);
                        if let Some(output) = st.outputs.pop_front() {
                            done = st.final_emitted && st.outputs.is_empty();
                            return Some((Ok(Bytes::from(output)), (stream, st, done)));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct OpenAiChatToLegacyCompletionsState {
    buffer: Vec<u8>,
    parser: SseParseState,
    response_id: String,
    model: String,
    created_at: i64,
    outputs: VecDeque<Vec<u8>>,
    native_passthrough: bool,
    latest_usage: Option<TokenUsage>,
    pending_finish_reason: Option<String>,
    final_emitted: bool,
}

impl OpenAiChatToLegacyCompletionsState {
    fn drain_frames(&mut self) {
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

    fn flush_pending_frame(&mut self) {
        if let Some(frame) = parse_sse_line("", &mut self.parser) {
            self.handle_frame(frame);
        }
    }

    fn handle_frame(&mut self, frame: SseFrame) {
        if frame.data.is_empty() {
            return;
        }

        if self.native_passthrough || frame_is_native_legacy_completions(&frame) {
            self.native_passthrough = true;
            self.outputs
                .push_back(format_sse_event(frame.event_name.as_deref(), &frame.data).into_bytes());
            if frame.data == "[DONE]" {
                self.final_emitted = true;
            }
            return;
        }

        if frame.data == "[DONE]" {
            self.emit_final_if_needed(None);
            return;
        }

        let Ok(chunk) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };

        if let Some(model) = chunk.get("model").and_then(|value| value.as_str()) {
            if !model.trim().is_empty() {
                self.model = model.to_string();
            }
        }

        if let Some(usage) = extract_stream_usage(&chunk) {
            self.latest_usage = Some(usage);
        }

        let choice = chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first());

        if let Some(choice) = choice {
            if let Some(text) = choice
                .get("delta")
                .and_then(|delta| delta.get("content"))
                .and_then(|value| value.as_str())
            {
                if !text.is_empty() {
                    let payload = build_legacy_completions_delta(
                        &self.response_id,
                        self.created_at,
                        &self.model,
                        text,
                    );
                    self.outputs
                        .push_back(format_sse_event(None, &payload.to_string()).into_bytes());
                }
            }

            if let Some(reason) = choice
                .get("finish_reason")
                .and_then(|value| value.as_str())
                .and_then(|value| map_openai_finish_reason(Some(value)))
            {
                self.pending_finish_reason = Some(reason);
                if self.latest_usage.is_some() {
                    self.emit_final_if_needed(None);
                }
            }
        }
    }

    fn emit_final_if_needed(&mut self, fallback_finish_reason: Option<&str>) {
        if self.final_emitted || self.native_passthrough {
            return;
        }

        let payload = build_legacy_completions_stop(
            &self.response_id,
            self.created_at,
            &self.model,
            self.latest_usage.as_ref(),
            self.pending_finish_reason
                .as_deref()
                .or(fallback_finish_reason),
        );
        self.outputs
            .push_back(format_sse_event(None, &payload.to_string()).into_bytes());
        self.outputs
            .push_back(format_sse_event(None, "[DONE]").into_bytes());
        self.final_emitted = true;
    }
}

fn frame_is_native_legacy_completions(frame: &SseFrame) -> bool {
    serde_json::from_str::<Value>(&frame.data)
        .ok()
        .and_then(|value| {
            value
                .get("choices")
                .and_then(|choices| choices.as_array())
                .and_then(|choices| {
                    choices.first().and_then(|choice| {
                        if choice.get("text").is_some() {
                            Some(true)
                        } else {
                            value
                                .get("object")
                                .and_then(|object| object.as_str())
                                .map(|object| object.starts_with("text_completion"))
                        }
                    })
                })
        })
        .unwrap_or(false)
}

fn extract_stream_usage(chunk: &Value) -> Option<TokenUsage> {
    let usage = chunk.get("usage")?;
    let prompt_tokens = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: usage
            .get("total_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(prompt_tokens + completion_tokens),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: usage
            .get("input_tokens_details")
            .and_then(|details| details.get("cached_tokens"))
            .and_then(|value| value.as_u64()),
    })
}

fn parse_role(role: &str) -> Result<MessageRole, GatewayError> {
    match role {
        "system" => Ok(MessageRole::System),
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "tool" | "function" => Ok(MessageRole::Tool),
        other => Err(GatewayError::bad_request(format!(
            "unknown message role: {other}"
        ))),
    }
}

fn parse_prompt_messages(value: &Value) -> Vec<CanonicalMessage> {
    match value {
        Value::String(text) => vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text { text: text.clone() }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        Value::Array(items) => items
            .iter()
            .map(|item| CanonicalMessage {
                role: MessageRole::User,
                content: vec![match item {
                    Value::String(text) => ContentPart::Text { text: text.clone() },
                    _ => ContentPart::Raw {
                        value: item.clone(),
                    },
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            })
            .collect(),
        other => vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Raw {
                value: other.clone(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
    }
}

fn parse_openai_content(content: Option<&Value>) -> Result<Vec<ContentPart>, GatewayError> {
    match content {
        None | Some(Value::Null) => Ok(vec![]),

        // Simple string content.
        Some(Value::String(s)) => Ok(vec![ContentPart::Text { text: s.clone() }]),

        // Array of content parts.
        Some(Value::Array(parts)) => {
            let mut out = Vec::with_capacity(parts.len());
            for part in parts {
                let kind = part.get("type").and_then(|v| v.as_str()).unwrap_or("text");

                match kind {
                    "text" => {
                        let text = part
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if is_plain_text_block(part) {
                            out.push(ContentPart::Text { text });
                        } else {
                            out.push(ContentPart::Raw {
                                value: part.clone(),
                            });
                        }
                    }
                    "image_url" => {
                        let url = part
                            .get("image_url")
                            .and_then(|u| u.get("url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let detail = part
                            .get("image_url")
                            .and_then(|u| u.get("detail"))
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        out.push(ContentPart::ImageUrl {
                            image_url: url,
                            detail,
                        });
                    }
                    _ => {
                        // Preserve unknown parts as Raw.
                        out.push(ContentPart::Raw {
                            value: part.clone(),
                        });
                    }
                }
            }
            Ok(out)
        }

        Some(other) => {
            // Fallback: treat any other JSON value as a raw part.
            Ok(vec![ContentPart::Raw {
                value: other.clone(),
            }])
        }
    }
}

fn parse_openai_message_tool_calls(message: &Value) -> Vec<CanonicalToolCall> {
    let tool_calls = parse_openai_tool_calls(message.get("tool_calls"));
    if !tool_calls.is_empty() {
        return tool_calls;
    }
    parse_openai_legacy_function_call(message.get("function_call"))
}

fn parse_openai_legacy_function_call(raw: Option<&Value>) -> Vec<CanonicalToolCall> {
    let Some(raw) = raw else {
        return vec![];
    };

    if !raw.is_object() {
        return vec![];
    }

    vec![CanonicalToolCall {
        id: raw
            .get("id")
            .or_else(|| raw.get("call_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        call_type: "function".to_string(),
        name: raw.get("name").and_then(|v| v.as_str()).map(str::to_string),
        arguments: raw.get("arguments").map(normalize_tool_arguments),
        raw: std::collections::HashMap::new(),
    }]
}

fn parse_openai_tool_calls(raw: Option<&Value>) -> Vec<CanonicalToolCall> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|tc| {
            let fn_obj = tc.get("function");
            CanonicalToolCall {
                id: tc.get("id").and_then(|v| v.as_str()).map(str::to_string),
                call_type: tc
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string(),
                name: fn_obj
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                arguments: fn_obj
                    .and_then(|f| f.get("arguments"))
                    .map(normalize_tool_arguments),
                raw: std::collections::HashMap::new(),
            }
        })
        .collect()
}

fn normalize_tool_arguments(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        decode_provider_tool_argument_entities(text)
    } else {
        serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
    }
}

fn decode_provider_tool_argument_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn map_openai_finish_reason(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "function_call" | "tool_use" => "tool_calls",
            other => other,
        }
        .to_string(),
    )
}

fn normalize_openai_wire_finish_reason(value: Option<&str>, has_tool_calls: bool) -> &'static str {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("completed") => {
            if has_tool_calls {
                "tool_calls"
            } else {
                "stop"
            }
        }
        Some("incomplete") | Some("length") | Some("max_tokens") => "length",
        Some("tool_calls") | Some("function_call") | Some("tool_use") => "tool_calls",
        Some("stop") | Some("end_turn") => "stop",
        Some(_) => "stop",
        None => {
            if has_tool_calls {
                "tool_calls"
            } else {
                "stop"
            }
        }
    }
}

fn pack_openai_tool_choice(source: ProtocolFamily, tool_choice: Option<&Value>) -> Option<Value> {
    let tool_choice = tool_choice?;
    if source == ProtocolFamily::OpenAi {
        return Some(tool_choice.clone());
    }

    let Some(object) = tool_choice.as_object() else {
        return Some(tool_choice.clone());
    };

    let choice_type = object.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match choice_type {
        "auto" => Some(json!("auto")),
        "none" => Some(json!("none")),
        "any" => Some(json!("required")),
        "tool" => object
            .get("name")
            .and_then(|v| v.as_str())
            .map(|name| json!({"type": "function", "function": {"name": name}})),
        _ => Some(tool_choice.clone()),
    }
}

fn parse_openai_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|t| {
            let fn_obj = t.get("function");
            CanonicalTool {
                tool_type: t
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("function")
                    .to_string(),
                name: fn_obj
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                description: fn_obj
                    .and_then(|f| f.get("description"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                input_schema: fn_obj.and_then(|f| f.get("parameters")).cloned(),
                raw: t
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

fn is_plain_text_block(part: &Value) -> bool {
    part.as_object().is_some_and(|map| {
        map.keys()
            .all(|key| matches!(key.as_str(), "type" | "text"))
    })
}

fn ensure_stream_usage_requested(body: &mut Value) {
    let Some(object) = body.as_object_mut() else {
        return;
    };

    match object.get_mut("stream_options") {
        Some(Value::Object(options)) => {
            options
                .entry("include_usage".to_string())
                .or_insert_with(|| json!(true));
        }
        Some(_) => {}
        None => {
            object.insert(
                "stream_options".to_string(),
                json!({ "include_usage": true }),
            );
        }
    }
}

fn pack_openai_message(msg: &CanonicalMessage) -> Value {
    let role = match msg.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    };

    let content: Value = if msg.role == MessageRole::Tool {
        Value::String(render_openai_tool_message_content(msg))
    } else if msg.content.len() == 1 {
        if let Some(text) = msg.content[0].as_text() {
            Value::String(text.to_string())
        } else {
            pack_content_parts_array(&msg.content)
        }
    } else if msg.content.is_empty() {
        Value::String(String::new())
    } else {
        pack_content_parts_array(&msg.content)
    };

    let mut obj = json!({
        "role": role,
        "content": content,
    });

    if let Some(name) = &msg.name {
        obj["name"] = json!(name);
    }

    if let Some(tcid) = &msg.tool_call_id {
        obj["tool_call_id"] = json!(tcid);
    }

    if !msg.tool_calls.is_empty() {
        obj["tool_calls"] = json!(msg
            .tool_calls
            .iter()
            .map(|tc| json!({
                "id": tc.id,
                "type": tc.call_type,
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments,
                }
            }))
            .collect::<Vec<_>>());
    }

    obj
}

fn render_openai_tool_message_content(msg: &CanonicalMessage) -> String {
    if msg.content.is_empty() {
        return String::new();
    }

    let mut fragments = Vec::new();
    for part in &msg.content {
        match part {
            ContentPart::Text { text } => fragments.push(text.clone()),
            ContentPart::Json { value } | ContentPart::Raw { value } => fragments.push(
                value
                    .get("text")
                    .and_then(|entry| entry.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string()),
            ),
            ContentPart::ImageUrl { image_url, .. } => {
                fragments.push(format!("[image omitted: {image_url}]"));
            }
        }
    }
    fragments.join("\n")
}

fn pack_content_parts_array(parts: &[ContentPart]) -> Value {
    Value::Array(
        parts
            .iter()
            .map(|p| match p {
                ContentPart::Text { text } => json!({"type": "text", "text": text}),
                ContentPart::ImageUrl { image_url, detail } => {
                    if let Some(d) = detail {
                        json!({"type": "image_url", "image_url": {"url": image_url, "detail": d}})
                    } else {
                        json!({"type": "image_url", "image_url": {"url": image_url}})
                    }
                }
                ContentPart::Json { value } => json!({"type": "json", "value": value}),
                ContentPart::Raw { value } => value.clone(),
            })
            .collect(),
    )
}

fn pack_openai_tool(tool: &CanonicalTool) -> Value {
    let mut fn_obj = json!({});
    if let Some(name) = &tool.name {
        fn_obj["name"] = json!(name);
    }
    if let Some(desc) = &tool.description {
        fn_obj["description"] = json!(desc);
    }
    if let Some(schema) = &tool.input_schema {
        fn_obj["parameters"] = schema.clone();
    }

    json!({
        "type": tool.tool_type,
        "function": fn_obj,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── normalize_chat_completions ────────────────────────────────────────

    #[test]
    fn normalize_basic_request() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "Be helpful."},
                {"role": "user", "content": "Hello!"}
            ],
            "stream": false,
        });
        let req = normalize_chat_completions(body).unwrap();
        assert_eq!(req.requested_model.as_deref(), Some("gpt-4o"));
        assert_eq!(req.messages.len(), 2);
        assert!(!req.stream);
        assert_eq!(req.messages[0].role, MessageRole::System);
        assert_eq!(req.messages[1].role, MessageRole::User);
    }

    #[test]
    fn normalize_array_content_parts() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Look at this image:"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
                ]
            }]
        });
        let req = normalize_chat_completions(body).unwrap();
        assert_eq!(req.messages[0].content.len(), 2);
        assert!(matches!(
            req.messages[0].content[0],
            ContentPart::Text { .. }
        ));
        assert!(matches!(
            req.messages[0].content[1],
            ContentPart::ImageUrl { .. }
        ));
    }

    #[test]
    fn normalize_captures_extra_fields() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "temperature": 0.7,
            "max_tokens": 1000,
            "top_p": 0.9,
        });
        let req = normalize_chat_completions(body).unwrap();
        assert_eq!(req.extra.get("temperature"), Some(&json!(0.7)));
        assert_eq!(req.extra.get("max_tokens"), Some(&json!(1000)));
        assert_eq!(req.extra.get("top_p"), Some(&json!(0.9)));
    }

    #[test]
    fn normalize_missing_messages_returns_error() {
        let body = json!({"model": "gpt-4o"});
        assert!(normalize_chat_completions(body).is_err());
    }

    // ── pack_openai ───────────────────────────────────────────────────────

    #[test]
    fn pack_standard_model_uses_max_tokens() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 512,
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(packed["max_tokens"], json!(512));
        assert!(
            packed.get("max_completion_tokens").is_none()
                || packed["max_completion_tokens"].is_null()
        );
    }

    #[test]
    fn pack_reasoning_model_uses_max_completion_tokens() {
        let body = json!({
            "model": "o1-preview",
            "messages": [{"role": "user", "content": "reason about this"}],
            "max_tokens": 2000,
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "o1-preview", false);
        assert_eq!(packed["max_completion_tokens"], json!(2000));
    }

    #[test]
    fn pack_inserts_system_message_first() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Hello"}
            ]
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        let msgs = packed["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are helpful.");
        assert_eq!(msgs[1]["role"], "user");
    }

    // ── unpack_openai_response ────────────────────────────────────────────

    #[test]
    fn unpack_standard_response() {
        let body = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello there!"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });
        let resp = unpack_openai_response(&body).unwrap();
        assert_eq!(resp.text, "Hello there!");
        assert_eq!(resp.model, "gpt-4o");
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
        let usage = resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn unpack_missing_choices_returns_error() {
        let body = json!({"model": "gpt-4o"});
        assert!(unpack_openai_response(&body).is_err());
    }

    // ── build_chat_completions_success ────────────────────────────────────

    #[test]
    fn build_success_response_structure() {
        let usage = TokenUsage {
            prompt_tokens: 5,
            completion_tokens: 10,
            total_tokens: 15,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        let resp = build_chat_completions_success(
            "chatcmpl-abc",
            1700000000,
            "gpt-4o",
            "Hi!",
            Some(&usage),
            &[],
            None,
        );
        assert_eq!(resp["id"], "chatcmpl-abc");
        assert_eq!(resp["object"], "chat.completion");
        assert_eq!(resp["choices"][0]["message"]["content"], "Hi!");
        assert_eq!(resp["choices"][0]["finish_reason"], "stop");
        assert_eq!(resp["usage"]["total_tokens"], 15);
    }

    #[test]
    fn build_success_response_propagates_finish_reason() {
        let resp = build_chat_completions_success(
            "chatcmpl-xyz",
            1700000000,
            "gpt-4o",
            "Done",
            None,
            &[],
            Some("tool_calls"),
        );
        assert_eq!(resp["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn build_success_response_defaults_to_tool_calls_when_tools_present() {
        let resp = build_chat_completions_success(
            "chatcmpl-tools",
            1700000000,
            "gpt-4o",
            "",
            None,
            &[CanonicalToolCall {
                id: Some("call_1".to_string()),
                call_type: "function".to_string(),
                name: Some("weather".to_string()),
                arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                raw: std::collections::HashMap::new(),
            }],
            None,
        );
        assert_eq!(resp["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn pack_tool_message_stringifies_json_content() {
        let message = CanonicalMessage {
            role: MessageRole::Tool,
            content: vec![ContentPart::Json {
                value: json!({"city":"Hangzhou","condition":"sunny"}),
            }],
            name: Some("weather".to_string()),
            tool_call_id: Some("call_weather".to_string()),
            tool_calls: vec![],
        };
        let packed = pack_openai_message(&message);
        assert_eq!(packed["role"], "tool");
        assert_eq!(
            packed["content"],
            json!("{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}")
        );
        assert_eq!(packed["tool_call_id"], "call_weather");
    }

    #[test]
    fn normalize_preserves_unknown_vendor_fields() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "temperature": 0.5,
            "my_vendor_flag": {"enabled": true},
            "another_ext": 42,
        });
        let req = normalize_chat_completions(body).unwrap();
        assert_eq!(
            req.extra.get("my_vendor_flag"),
            Some(&json!({"enabled": true}))
        );
        assert_eq!(req.extra.get("another_ext"), Some(&json!(42)));
        assert_eq!(req.extra.get("temperature"), Some(&json!(0.5)));
    }

    #[test]
    fn pack_unknown_vendor_fields_roundtrip() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "my_vendor_flag": true,
        });
        let req = normalize_chat_completions(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(packed["my_vendor_flag"], json!(true));
    }

    #[test]
    fn normalize_legacy_completions_maps_prompt_to_messages() {
        let req = normalize_legacy_completions(json!({
            "model": "gpt-3.5-turbo-instruct",
            "prompt": "finish this sentence",
            "stream": true,
            "user": "user-123"
        }))
        .unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::Completions);
        assert!(req.stream);
        assert_eq!(
            req.requested_model.as_deref(),
            Some("gpt-3.5-turbo-instruct")
        );
        assert_eq!(req.messages[0].text_content(), "finish this sentence");
        assert_eq!(req.explicit_session_key.as_deref(), Some("user-123"));
    }

    #[test]
    fn normalize_embeddings_maps_input_to_messages() {
        let req = normalize_embeddings(json!({
            "model": "text-embedding-3-large",
            "input": ["alpha", "beta"],
            "user": "embed-user"
        }))
        .unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::Embeddings);
        assert_eq!(
            req.requested_model.as_deref(),
            Some("text-embedding-3-large")
        );
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].text_content(), "alpha");
        assert_eq!(req.messages[1].text_content(), "beta");
        assert_eq!(req.explicit_session_key.as_deref(), Some("embed-user"));
    }

    #[test]
    fn normalize_audio_speech_maps_input_to_messages() {
        let req = normalize_audio_speech(json!({
            "model": "tts-1",
            "input": "say hello"
        }))
        .unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::AudioSpeech);
        assert_eq!(req.requested_model.as_deref(), Some("tts-1"));
        assert_eq!(req.messages[0].text_content(), "say hello");
    }

    #[test]
    fn normalize_audio_transcriptions_uses_prompt_or_filename() {
        let req = normalize_audio_transcriptions(json!({
            "model": "whisper-1",
            "prompt": "medical dictation",
            "file": {
                "file_name": "note.wav",
                "mime_type": "audio/wav",
                "base64": "ZmFrZQ=="
            }
        }))
        .unwrap();
        assert_eq!(req.endpoint_kind, EndpointKind::AudioTranscriptions);
        assert_eq!(req.requested_model.as_deref(), Some("whisper-1"));
        assert_eq!(req.messages[0].text_content(), "medical dictation");
    }

    #[test]
    fn unpack_response_with_array_content() {
        let body = json!({
            "id": "chatcmpl-arr",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": [
                        {"type": "text", "text": "Hello"},
                        {"type": "text", "text": " world"}
                    ]
                },
                "finish_reason": "stop"
            }]
        });
        let resp = unpack_openai_response(&body).unwrap();
        assert_eq!(resp.text, "Hello world");
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn unpack_response_preserves_tool_calls() {
        let body = json!({
            "id": "chatcmpl-tc",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"NYC\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let resp = unpack_openai_response(&body).unwrap();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].id.as_deref(), Some("call_abc"));
        assert_eq!(resp.tool_calls[0].name.as_deref(), Some("get_weather"));
        assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn unpack_response_unescapes_tool_call_argument_entities() {
        let body = json!({
            "id": "chatcmpl-escaped",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_html",
                        "type": "function",
                        "function": {
                            "name": "weather",
                            "arguments": "{&quot;city&quot;:&quot;Hangzhou&quot;}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let resp = unpack_openai_response(&body).unwrap();
        assert_eq!(
            resp.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    #[test]
    fn unpack_response_parses_legacy_function_call() {
        let body = json!({
            "id": "chatcmpl-fc",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "function_call": {
                        "name": "lookup_weather",
                        "arguments": {"city": "Shanghai"}
                    }
                },
                "finish_reason": "function_call"
            }]
        });
        let resp = unpack_openai_response(&body).unwrap();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name.as_deref(), Some("lookup_weather"));
        assert_eq!(
            resp.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Shanghai\"}")
        );
        assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn unpack_response_promotes_xml_tool_calls_from_text() {
        let body = json!({
            "id": "chatcmpl-xml",
            "model": "xop35qwen2b",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>"
                },
                "finish_reason": "stop"
            }]
        });
        let resp = unpack_openai_response(&body).unwrap();
        assert!(resp.text.is_empty());
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn pack_maps_anthropic_tool_choice_any_to_required() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "tool_choice": {"type": "any"}
        });
        let req = crate::protocol::anthropic::normalize_messages(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(packed["tool_choice"], json!("required"));
    }

    #[test]
    fn pack_maps_anthropic_specific_tool_choice_to_openai_function_object() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "tool_choice": {"type": "tool", "name": "weather"}
        });
        let req = crate::protocol::anthropic::normalize_messages(body).unwrap();
        let packed = pack_openai(&req, "gpt-4o", false);
        assert_eq!(
            packed["tool_choice"],
            json!({"type": "function", "function": {"name": "weather"}})
        );
    }

    // ── build_chat_completions_delta ──────────────────────────────────────

    #[test]
    fn build_delta_chunk_structure() {
        let chunk = build_chat_completions_delta("chatcmpl-abc", 1700000000, "gpt-4o", "Hello");
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["choices"][0]["delta"]["content"], "Hello");
        assert_eq!(chunk["choices"][0]["finish_reason"], Value::Null);
    }

    // ── build_chat_completions_stop ───────────────────────────────────────

    #[test]
    fn build_stop_chunk_structure() {
        let chunk = build_chat_completions_stop("chatcmpl-abc", 1700000000, "gpt-4o", None);
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["choices"][0]["finish_reason"], "stop");
    }

    #[test]
    fn build_legacy_completion_success_structure() {
        let usage = TokenUsage {
            prompt_tokens: 4,
            completion_tokens: 6,
            total_tokens: 10,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        let resp = build_legacy_completions_success(
            "cmpl-abc",
            1700000000,
            "gpt-5.4",
            "legacy ok",
            Some(&usage),
            None,
        );
        assert_eq!(resp["object"], "text_completion");
        assert_eq!(resp["choices"][0]["text"], "legacy ok");
        assert_eq!(resp["choices"][0]["finish_reason"], "stop");
        assert_eq!(resp["usage"]["total_tokens"], 10);
    }

    #[test]
    fn build_legacy_completion_stop_uses_custom_finish_reason() {
        let resp =
            build_legacy_completions_stop("cmpl-abc", 1700000000, "gpt-5.4", None, Some("length"));
        assert_eq!(resp["choices"][0]["finish_reason"], "length");
    }
}
