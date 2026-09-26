use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};

use crate::protocol::canonical::{CanonicalTool, CanonicalToolCall};

use super::parser::parse_tool_calls_from_text_with_context;

const MAX_TOOL_DETECTION_ACCUMULATION_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOOL_DETECTION_CHUNKS: usize = 65_536;
const MAX_RETAINED_SSE_LINE_CAPACITY_BYTES: usize = 64 * 1024;

// XML opening tags that trigger capture mode in the streaming detector.
const TOOL_CALL_OPEN_TAGS: &[&str] = &[
    "<tool_calls>",
    "<tool_calls ",
    "<function_calls>",
    "<function_calls ",
    "<tool_call>",
    "<tool_call ",
    "<invoke ",
    "<invoke>",
];

/// Check whether `text` contains an XML opening tag that indicates the start
/// of a tool call block.
pub(super) fn has_tool_call_opening(text: &str) -> bool {
    TOOL_CALL_OPEN_TAGS.iter().any(|tag| text.contains(tag))
}

/// Extract the `content` string from an OpenAI SSE `data:` line.
///
/// Expected format: `data: {"choices":[{"delta":{"content":"..."}}]}`
/// Returns `None` for `[DONE]`, empty lines, or missing content field.
pub(super) fn extract_sse_content(line: &str) -> Option<String> {
    let data = line
        .strip_prefix("data:")
        .or_else(|| line.strip_prefix("data: "))?;
    let data = data.trim();
    if data == "[DONE]" || data.is_empty() {
        return None;
    }
    let parsed: Value = serde_json::from_str(data).ok()?;
    parsed
        .get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()
        .map(|s| s.to_string())
}

/// Build an SSE chunk that carries OpenAI-format `tool_calls` deltas.
///
/// This produces a single SSE frame containing ALL tool calls at once
/// (matching the OpenAI convention for the first chunk that announces
/// tool calls).
pub(super) fn build_tool_calls_sse_chunk(
    tool_calls: &[CanonicalToolCall],
    model: &str,
    response_id: &str,
) -> Vec<u8> {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let tc_json: Vec<Value> = tool_calls
        .iter()
        .enumerate()
        .map(|(i, tc)| {
            json!({
                "index": i,
                "id": tc.id,
                "type": tc.call_type,
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments.as_deref().unwrap_or("{}")
                }
            })
        })
        .collect();

    let chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": null,
                "tool_calls": tc_json,
            },
            "finish_reason": "tool_calls",
        }]
    });

    format!("data: {}\n\n", chunk).into_bytes()
}

/// Wrap a byte stream to detect and translate XML tool calls.
///
/// When `tools_were_injected` is true, the model is expected to output
/// tool calls as its entire response (not mixed with regular text).
/// This wrapper accumulates bounded text content from the SSE stream, then
/// on stream end checks for XML tool calls. If found, it emits
/// OpenAI-format `tool_calls` chunks + `[DONE]` instead of the raw text.
/// If no tool calls are found, it replays the original chunks unmodified.
///
/// This "accumulate then decide" strategy is correct because tool-injected
/// models output tool calls as their ENTIRE response.
pub fn wrap_streaming_tool_detection(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    response_id: String,
    tools: Vec<CanonicalTool>,
    tool_choice: Option<Value>,
    conversation_hint: Option<String>,
) -> std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static>> {
    wrap_streaming_tool_detection_with_limits(
        inner,
        model,
        response_id,
        tools,
        tool_choice,
        conversation_hint,
        MAX_TOOL_DETECTION_ACCUMULATION_BYTES,
        MAX_TOOL_DETECTION_ACCUMULATION_BYTES,
        MAX_TOOL_DETECTION_CHUNKS,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_streaming_tool_detection_with_limits(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    response_id: String,
    tools: Vec<CanonicalTool>,
    tool_choice: Option<Value>,
    conversation_hint: Option<String>,
    max_original_bytes: usize,
    max_accumulated_text_bytes: usize,
    max_chunks: usize,
) -> std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static>> {
    let state = ToolDetectorState {
        model,
        response_id,
        tools,
        tool_choice,
        conversation_hint,
        accumulated_text: String::new(),
        pending_sse_line: Vec::new(),
        original_chunks: Vec::new(),
        original_bytes: 0,
        max_original_bytes,
        max_accumulated_text_bytes,
        max_chunks,
        done: false,
        replay_index: 0,
        emit_queue: Vec::new(),
        emit_index: 0,
    };

    Box::pin(futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
        ),
        |(mut stream, mut st)| async move {
            use futures::StreamExt;

            // Phase 3: emitting tool_calls chunks (after detection succeeded).
            if !st.emit_queue.is_empty() {
                if st.emit_index < st.emit_queue.len() {
                    let chunk = std::mem::take(&mut st.emit_queue[st.emit_index]);
                    st.emit_index += 1;
                    return Some((Ok(Bytes::from(chunk)), (stream, st)));
                }
                return None; // All emitted.
            }

            // Phase 2: replaying original chunks (after detection found no tool calls).
            if st.done && st.replay_index > 0 {
                if st.replay_index <= st.original_chunks.len() {
                    let idx = st.replay_index - 1;
                    st.replay_index += 1;
                    if idx < st.original_chunks.len() {
                        let chunk = std::mem::take(&mut st.original_chunks[idx]);
                        return Some((Ok(chunk), (stream, st)));
                    }
                }
                return None;
            }

            // Phase 1: accumulate all chunks from the inner stream.
            if !st.done {
                loop {
                    match stream.next().await {
                        Some(Ok(chunk)) => {
                            if let Err(error) = st.capture_chunk(chunk) {
                                st.fail_and_release();
                                return Some((Err(error), (stream, st)));
                            }
                        }
                        Some(Err(e)) => {
                            // Error — forward immediately.
                            st.fail_and_release();
                            return Some((Err(e), (stream, st)));
                        }
                        None => {
                            // Inner stream ended — decide what to do.
                            st.done = true;
                            if let Err(error) = st.flush_pending_sse_line() {
                                st.fail_and_release();
                                return Some((Err(error), (stream, st)));
                            }
                            break;
                        }
                    }
                }

                // Stream finished. Check if accumulated text has tool calls.
                if has_tool_call_opening(&st.accumulated_text) {
                    let parse_result = parse_tool_calls_from_text_with_context(
                        &st.accumulated_text,
                        &st.tools,
                        st.tool_choice.as_ref(),
                        st.conversation_hint.as_deref(),
                    );
                    if parse_result.had_tool_calls {
                        // Build tool_calls SSE chunks.
                        let tc_chunk = build_tool_calls_sse_chunk(
                            &parse_result.tool_calls,
                            &st.model,
                            &st.response_id,
                        );
                        st.emit_queue.push(tc_chunk);
                        st.emit_queue.push(b"data: [DONE]\n\n".to_vec());
                        st.emit_index = 0;
                        st.release_detection_buffers();

                        // Emit the first chunk.
                        let first = std::mem::take(&mut st.emit_queue[st.emit_index]);
                        st.emit_index += 1;
                        return Some((Ok(Bytes::from(first)), (stream, st)));
                    }
                }

                // No tool calls — replay original chunks.
                if st.original_chunks.is_empty() {
                    return None;
                }
                st.accumulated_text = String::new();
                let first = std::mem::take(&mut st.original_chunks[0]);
                st.replay_index = 2; // Next call will read index 1.
                return Some((Ok(first), (stream, st)));
            }

            None
        },
    ))
}

/// Internal state for the streaming tool call detector.
struct ToolDetectorState {
    model: String,
    response_id: String,
    tools: Vec<CanonicalTool>,
    tool_choice: Option<Value>,
    conversation_hint: Option<String>,
    /// All text content extracted from SSE `content` fields.
    accumulated_text: String,
    /// Incomplete SSE line carried across arbitrary transport chunk boundaries.
    pending_sse_line: Vec<u8>,
    /// Original byte chunks saved for replay if no tool calls are detected.
    original_chunks: Vec<Bytes>,
    /// Total bytes retained by original_chunks.
    original_bytes: usize,
    max_original_bytes: usize,
    max_accumulated_text_bytes: usize,
    max_chunks: usize,
    /// Whether the inner stream has ended.
    done: bool,
    /// Index for replaying original_chunks (1-based, 0 means not replaying).
    replay_index: usize,
    /// Pre-built tool_calls SSE chunks to emit (when tool calls detected).
    emit_queue: Vec<Vec<u8>>,
    /// Index into emit_queue for emission.
    emit_index: usize,
}

impl ToolDetectorState {
    fn capture_chunk(&mut self, chunk: Bytes) -> Result<(), rquest::Error> {
        if self.original_chunks.len() >= self.max_chunks {
            return Err(tool_detection_stream_error(
                "tool_injection_stream_too_large",
                format!(
                    "stream exceeded the {}-chunk tool detection limit",
                    self.max_chunks
                ),
            ));
        }
        let next_original_bytes = self
            .original_bytes
            .checked_add(chunk.len())
            .filter(|size| *size <= self.max_original_bytes)
            .ok_or_else(|| {
                tool_detection_stream_error(
                    "tool_injection_stream_too_large",
                    format!(
                        "stream exceeded the {}-byte tool detection replay limit",
                        self.max_original_bytes
                    ),
                )
            })?;
        self.original_chunks.try_reserve_exact(1).map_err(|error| {
            tool_detection_stream_error(
                "tool_injection_stream_buffer_allocation_failed",
                format!("failed to reserve tool detection replay buffer: {error}"),
            )
        })?;

        self.capture_sse_content(&chunk)?;

        self.original_chunks.push(chunk);
        self.original_bytes = next_original_bytes;
        Ok(())
    }

    fn capture_sse_content(&mut self, chunk: &[u8]) -> Result<(), rquest::Error> {
        let mut start = 0;
        while let Some(relative_end) = chunk[start..].iter().position(|byte| *byte == b'\n') {
            let end = start + relative_end;
            if self.pending_sse_line.is_empty() {
                self.capture_sse_line(&chunk[start..end])?;
            } else {
                self.append_pending_sse_line(&chunk[start..end])?;
                self.flush_pending_sse_line()?;
            }
            start = end + 1;
        }
        if start < chunk.len() {
            self.append_pending_sse_line(&chunk[start..])?;
        }
        Ok(())
    }

    fn append_pending_sse_line(&mut self, segment: &[u8]) -> Result<(), rquest::Error> {
        self.pending_sse_line
            .len()
            .checked_add(segment.len())
            .filter(|size| *size <= self.max_original_bytes)
            .ok_or_else(|| {
                tool_detection_stream_error(
                    "tool_injection_stream_too_large",
                    format!(
                        "SSE line exceeded the {}-byte tool detection limit",
                        self.max_original_bytes
                    ),
                )
            })?;
        self.pending_sse_line
            .try_reserve_exact(segment.len())
            .map_err(|error| {
                tool_detection_stream_error(
                    "tool_injection_stream_buffer_allocation_failed",
                    format!("failed to reserve tool detection SSE line buffer: {error}"),
                )
            })?;
        self.pending_sse_line.extend_from_slice(segment);
        Ok(())
    }

    fn flush_pending_sse_line(&mut self) -> Result<(), rquest::Error> {
        if self.pending_sse_line.is_empty() {
            return Ok(());
        }
        let mut line = std::mem::take(&mut self.pending_sse_line);
        let result = self.capture_sse_line(&line);
        if line.capacity() <= MAX_RETAINED_SSE_LINE_CAPACITY_BYTES {
            line.clear();
            self.pending_sse_line = line;
        }
        result
    }

    fn capture_sse_line(&mut self, line: &[u8]) -> Result<(), rquest::Error> {
        let Ok(line) = std::str::from_utf8(line) else {
            return Ok(());
        };
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }
        if let Some(content) = extract_sse_content(line) {
            self.append_content(&content)?;
        }
        Ok(())
    }

    fn append_content(&mut self, content: &str) -> Result<(), rquest::Error> {
        self.accumulated_text
            .len()
            .checked_add(content.len())
            .filter(|size| *size <= self.max_accumulated_text_bytes)
            .ok_or_else(|| {
                tool_detection_stream_error(
                    "tool_injection_stream_too_large",
                    format!(
                        "content exceeded the {}-byte tool detection limit",
                        self.max_accumulated_text_bytes
                    ),
                )
            })?;
        self.accumulated_text
            .try_reserve_exact(content.len())
            .map_err(|error| {
                tool_detection_stream_error(
                    "tool_injection_stream_buffer_allocation_failed",
                    format!("failed to reserve tool detection content buffer: {error}"),
                )
            })?;
        self.accumulated_text.push_str(content);
        Ok(())
    }

    fn release_detection_buffers(&mut self) {
        self.accumulated_text = String::new();
        self.pending_sse_line = Vec::new();
        self.original_chunks = Vec::new();
        self.original_bytes = 0;
    }

    fn fail_and_release(&mut self) {
        self.done = true;
        self.release_detection_buffers();
        self.emit_queue = Vec::new();
        self.replay_index = 0;
        self.emit_index = 0;
    }
}

fn tool_detection_stream_error(code: &str, message: String) -> rquest::Error {
    let source = std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("{code}: {message}"),
    );
    rquest::Error::from(serde_json::Error::io(source))
}
