// ---------------------------------------------------------------------------
// upstream/stream.rs — TrackedStream
//
// A pinned Stream wrapper that records first-token latency, chunk count,
// byte count, and total duration, then fires a callback when the stream
// terminates (either naturally or via Drop).
// ---------------------------------------------------------------------------

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

use crate::protocol::canonical::TokenUsage;
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

// ---------------------------------------------------------------------------
// StreamMetrics
// ---------------------------------------------------------------------------

/// Metrics collected while reading a streaming upstream response.
#[derive(Debug, Clone)]
pub struct StreamMetrics {
    /// Milliseconds between the request being sent and the first
    /// non-empty chunk arriving. `None` if no chunks were received.
    pub first_token_latency_ms: Option<u64>,
    /// Number of `Bytes` chunks yielded by the stream.
    pub chunk_count: u64,
    /// Total bytes received across all chunks.
    pub total_bytes: u64,
    /// Milliseconds between stream construction and stream termination.
    pub total_duration_ms: u64,
}

// ---------------------------------------------------------------------------
// TrackedStream
// ---------------------------------------------------------------------------

/// A [`Stream`] adapter that transparently wraps an upstream byte stream and
/// collects metrics, calling a user-supplied callback when the stream ends.
///
/// The callback receives:
/// - The accumulated [`StreamMetrics`].
/// - A `bool` indicating whether the stream completed without error (`true`)
///   or was terminated by an error or a `Drop` before completion (`false`).
pub struct TrackedStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    started_at: Instant,
    first_token_seen: bool,
    first_token_latency_ms: Option<u64>,
    chunk_count: u64,
    total_bytes: u64,
    /// The callback to fire exactly once when the stream ends.
    on_complete: Option<Box<dyn FnOnce(StreamMetrics, bool) + Send>>,
    /// Set to `true` after `on_complete` has been called, preventing double-fire.
    completed: bool,
}

struct UsageTappedStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    capture: SseUsageCapture,
}

struct CompletionSemanticsTappedStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    capture: SseCompletionSemanticsCapture,
}

struct ArchiveTappedStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    capture: StreamArchiveCapture,
}

#[derive(Debug, Clone)]
pub struct StreamArchiveSnapshot {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct StreamArchiveHandle {
    state: Arc<Mutex<StreamArchiveCaptureState>>,
}

#[derive(Debug)]
struct SseUsageCapture {
    latest_usage: Arc<Mutex<Option<TokenUsage>>>,
    buffer: Vec<u8>,
    pending_event: Option<String>,
    pending_data_lines: Vec<String>,
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

#[derive(Debug)]
struct SseCompletionSemanticsCapture {
    latest_completion_semantics: Arc<Mutex<Option<String>>>,
    buffer: Vec<u8>,
    parser: SseParseState,
}

#[derive(Debug)]
struct StreamArchiveCapture {
    state: Arc<Mutex<StreamArchiveCaptureState>>,
}

#[derive(Debug)]
struct StreamArchiveCaptureState {
    bytes: Vec<u8>,
    truncated: bool,
    max_bytes: usize,
}

impl TrackedStream {
    /// Wrap `inner` in a tracked stream.
    ///
    /// `on_complete` is called exactly once, either when the stream returns
    /// `Poll::Ready(None)`, when an error item is yielded, or when the
    /// `TrackedStream` is dropped.
    pub fn new(
        inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
        on_complete: impl FnOnce(StreamMetrics, bool) + Send + 'static,
    ) -> Self {
        Self::new_with_started_at(inner, Instant::now(), on_complete)
    }

    /// Wrap `inner` in a tracked stream using the instant at which the real
    /// upstream attempt began, which may precede construction of this wrapper.
    pub fn new_with_started_at(
        inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
        started_at: Instant,
        on_complete: impl FnOnce(StreamMetrics, bool) + Send + 'static,
    ) -> Self {
        Self {
            inner: Box::pin(inner),
            started_at,
            first_token_seen: false,
            first_token_latency_ms: None,
            chunk_count: 0,
            total_bytes: 0,
            on_complete: Some(Box::new(on_complete)),
            completed: false,
        }
    }

    /// Return a snapshot of the metrics accumulated so far.
    pub fn metrics(&self) -> StreamMetrics {
        StreamMetrics {
            first_token_latency_ms: self.first_token_latency_ms,
            chunk_count: self.chunk_count,
            total_bytes: self.total_bytes,
            total_duration_ms: self.started_at.elapsed().as_millis() as u64,
        }
    }

    /// Fire the callback (if not already fired) with the current metrics and
    /// the given success flag.
    fn fire_callback(&mut self, success: bool) {
        if self.completed {
            return;
        }
        self.completed = true;
        if let Some(cb) = self.on_complete.take() {
            cb(self.metrics(), success);
        }
    }
}

pub fn tap_sse_usage(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    Arc<Mutex<Option<TokenUsage>>>,
) {
    let latest_usage = Arc::new(Mutex::new(None));
    let capture = SseUsageCapture::new(Arc::clone(&latest_usage));
    (
        UsageTappedStream {
            inner: Box::pin(inner),
            capture,
        },
        latest_usage,
    )
}

pub fn snapshot_tapped_usage(latest_usage: &Arc<Mutex<Option<TokenUsage>>>) -> Option<TokenUsage> {
    latest_usage.lock().ok().and_then(|usage| (*usage).clone())
}

pub fn tap_sse_completion_semantics(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    Arc<Mutex<Option<String>>>,
) {
    let latest_completion_semantics = Arc::new(Mutex::new(None));
    let capture = SseCompletionSemanticsCapture::new(Arc::clone(&latest_completion_semantics));
    (
        CompletionSemanticsTappedStream {
            inner: Box::pin(inner),
            capture,
        },
        latest_completion_semantics,
    )
}

pub fn snapshot_tapped_completion_semantics(
    latest_completion_semantics: &Arc<Mutex<Option<String>>>,
) -> Option<String> {
    latest_completion_semantics
        .lock()
        .ok()
        .and_then(|value| (*value).clone())
}

pub fn tap_stream_archive(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    max_bytes: usize,
) -> (
    impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    StreamArchiveHandle,
) {
    let state = Arc::new(Mutex::new(StreamArchiveCaptureState {
        bytes: Vec::new(),
        truncated: false,
        max_bytes,
    }));
    (
        ArchiveTappedStream {
            inner: Box::pin(inner),
            capture: StreamArchiveCapture {
                state: Arc::clone(&state),
            },
        },
        StreamArchiveHandle { state },
    )
}

pub fn snapshot_tapped_archive(handle: &StreamArchiveHandle) -> StreamArchiveSnapshot {
    let Ok(state) = handle.state.lock() else {
        return StreamArchiveSnapshot {
            text: String::new(),
            truncated: true,
        };
    };
    StreamArchiveSnapshot {
        text: String::from_utf8_lossy(&state.bytes).into_owned(),
        truncated: state.truncated,
    }
}

// ---------------------------------------------------------------------------
// Stream impl
// ---------------------------------------------------------------------------

impl Stream for TrackedStream {
    type Item = Result<Bytes, rquest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // SAFETY: we never move `inner` out of the struct.
        let inner = unsafe { self.as_mut().map_unchecked_mut(|s| &mut s.inner) };

        match inner.poll_next(cx) {
            Poll::Pending => Poll::Pending,

            Poll::Ready(None) => {
                // Stream ended cleanly.
                self.fire_callback(true);
                Poll::Ready(None)
            }

            Poll::Ready(Some(Ok(bytes))) => {
                let len = bytes.len() as u64;

                // Record first-token latency on the first non-empty chunk.
                if !self.first_token_seen && len > 0 {
                    self.first_token_seen = true;
                    self.first_token_latency_ms =
                        Some(self.started_at.elapsed().as_millis() as u64);
                }

                self.chunk_count += 1;
                self.total_bytes += len;

                Poll::Ready(Some(Ok(bytes)))
            }

            Poll::Ready(Some(Err(e))) => {
                // Error terminates the stream.
                self.fire_callback(false);
                Poll::Ready(Some(Err(e)))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Drop
// ---------------------------------------------------------------------------

impl Drop for TrackedStream {
    fn drop(&mut self) {
        // Ensure callback fires even if the stream is dropped before exhaustion
        // (e.g., the client disconnected).
        self.fire_callback(false);
    }
}

impl Stream for UsageTappedStream {
    type Item = Result<Bytes, rquest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = unsafe { self.as_mut().map_unchecked_mut(|stream| &mut stream.inner) };

        match inner.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(bytes))) => {
                self.capture.feed(&bytes);
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}

impl Stream for CompletionSemanticsTappedStream {
    type Item = Result<Bytes, rquest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = unsafe { self.as_mut().map_unchecked_mut(|stream| &mut stream.inner) };

        match inner.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(bytes))) => {
                self.capture.feed(&bytes);
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}

impl Stream for ArchiveTappedStream {
    type Item = Result<Bytes, rquest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let inner = unsafe { self.as_mut().map_unchecked_mut(|stream| &mut stream.inner) };

        match inner.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(bytes))) => {
                self.capture.feed(&bytes);
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}

impl SseUsageCapture {
    fn new(latest_usage: Arc<Mutex<Option<TokenUsage>>>) -> Self {
        Self {
            latest_usage,
            buffer: Vec::new(),
            pending_event: None,
            pending_data_lines: Vec::new(),
            prompt_tokens: None,
            completion_tokens: None,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }
    }

    fn feed(&mut self, chunk: &Bytes) {
        self.buffer.extend_from_slice(chunk);

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
            self.process_line(line);
        }
    }

    fn process_line(&mut self, line: &str) {
        if line.is_empty() {
            self.finish_event();
            return;
        }

        if let Some(event) = line.strip_prefix("event:") {
            self.pending_event = Some(event.trim().to_string());
            return;
        }

        if let Some(data) = line.strip_prefix("data:") {
            self.pending_data_lines.push(data.trim_start().to_string());
        }
    }

    fn finish_event(&mut self) {
        if self.pending_event.is_none() && self.pending_data_lines.is_empty() {
            return;
        }

        let event = self.pending_event.take();
        let data = self.pending_data_lines.join("\n");
        self.pending_data_lines.clear();

        if data.is_empty() || data == "[DONE]" {
            return;
        }

        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            return;
        };

        if event.as_deref() == Some("message_start") {
            if let Some(usage) = value
                .get("message")
                .and_then(|message| message.get("usage"))
                .and_then(parse_usage_value)
            {
                self.merge_usage(usage);
            }
            return;
        }

        if let Some(usage) = value.get("usage").and_then(parse_usage_value) {
            self.merge_usage(usage);
        }
        if let Some(usage) = value
            .get("response")
            .and_then(|response| response.get("usage"))
            .and_then(parse_usage_value)
        {
            self.merge_usage(usage);
        }
    }

    fn merge_usage(&mut self, usage: TokenUsage) {
        if usage.prompt_tokens > 0 {
            self.prompt_tokens = Some(usage.prompt_tokens);
        }
        if usage.completion_tokens > 0 {
            self.completion_tokens = Some(usage.completion_tokens);
        }
        if let Some(value) = usage.cache_creation_input_tokens {
            self.cache_creation_input_tokens = Some(value);
        }
        if let Some(value) = usage.cache_read_input_tokens {
            self.cache_read_input_tokens = Some(value);
        }

        let merged = TokenUsage {
            prompt_tokens: self.prompt_tokens.unwrap_or(usage.prompt_tokens),
            completion_tokens: self.completion_tokens.unwrap_or(usage.completion_tokens),
            total_tokens: self
                .prompt_tokens
                .unwrap_or(usage.prompt_tokens)
                .saturating_add(self.completion_tokens.unwrap_or(usage.completion_tokens)),
            cache_creation_input_tokens: self.cache_creation_input_tokens,
            cache_read_input_tokens: self.cache_read_input_tokens,
        };

        if let Ok(mut latest_usage) = self.latest_usage.lock() {
            *latest_usage = Some(merged);
        }
    }
}

impl SseCompletionSemanticsCapture {
    fn new(latest_completion_semantics: Arc<Mutex<Option<String>>>) -> Self {
        Self {
            latest_completion_semantics,
            buffer: Vec::new(),
            parser: SseParseState::new(),
        }
    }

    fn feed(&mut self, chunk: &Bytes) {
        self.buffer.extend_from_slice(chunk);

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
                self.process_frame(frame.event_name.as_deref(), &frame.data);
            }
        }
    }

    fn process_frame(&mut self, event_name: Option<&str>, data: &str) {
        if data.is_empty() || data == "[DONE]" {
            return;
        }

        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return;
        };

        let Some(semantics) = extract_completion_semantics(event_name, &value) else {
            return;
        };

        if let Ok(mut latest_completion_semantics) = self.latest_completion_semantics.lock() {
            *latest_completion_semantics = Some(semantics.to_string());
        }
    }
}

impl StreamArchiveCapture {
    fn feed(&mut self, chunk: &Bytes) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let remaining = state.max_bytes.saturating_sub(state.bytes.len());
        if remaining == 0 {
            state.truncated = true;
            return;
        }
        if chunk.len() > remaining {
            state.bytes.extend_from_slice(&chunk[..remaining]);
            state.truncated = true;
        } else {
            state.bytes.extend_from_slice(chunk);
        }
    }
}

fn extract_completion_semantics(event_name: Option<&str>, value: &Value) -> Option<&'static str> {
    let payload_type = value.get("type").and_then(|entry| entry.as_str());

    if matches!(
        event_name.or(payload_type),
        Some("response.completed") | Some("responseCompleted")
    ) {
        if let Some(response) = value.get("response") {
            return extract_responses_completion_semantics(response);
        }
    }

    if let Some(response_semantics) = extract_responses_completion_semantics(value) {
        return Some(response_semantics);
    }

    if matches!(
        event_name.or(payload_type),
        Some("message_delta") | Some("messageDelta")
    ) {
        if let Some(reason) = value
            .get("delta")
            .and_then(|delta| delta.get("stop_reason").or_else(|| delta.get("stopReason")))
            .and_then(|entry| entry.as_str())
        {
            return map_completion_reason(reason);
        }
    }

    value
        .get("choices")
        .and_then(|entry| entry.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(|entry| entry.as_str())
        .and_then(map_completion_reason)
}

fn extract_responses_completion_semantics(value: &Value) -> Option<&'static str> {
    let has_tool_calls = value
        .get("output")
        .and_then(|entry| entry.as_array())
        .map(|items| {
            items.iter().any(|item| {
                matches!(
                    item.get("type").and_then(|entry| entry.as_str()),
                    Some("function_call") | Some("custom_tool_call")
                )
            })
        })
        .unwrap_or(false);

    if has_tool_calls {
        return Some("tool_calls");
    }

    value
        .get("status")
        .and_then(|entry| entry.as_str())
        .and_then(map_completion_reason)
}

fn map_completion_reason(value: &str) -> Option<&'static str> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Some(match value {
        "stop" | "completed" | "complete" | "end_turn" | "stop_sequence" | "COMPLETE" => "stop",
        "tool_calls" | "function_call" | "tool_use" | "tool_call" | "TOOL_CALL" => "tool_calls",
        "length" | "max_tokens" | "incomplete" | "MAX_TOKENS" => "length",
        "content_filter" | "content_filtered" | "guardrail_intervened" | "SAFETY" | "safety" => {
            "content_filter"
        }
        _ => "other_provider_reason",
    })
}

fn parse_usage_value(value: &Value) -> Option<TokenUsage> {
    if let Some(output) = value.get("output_tokens").and_then(|entry| entry.as_u64()) {
        let input = value
            .get("input_tokens")
            .and_then(|entry| entry.as_u64())
            .unwrap_or(0);
        let total_tokens = value
            .get("total_tokens")
            .and_then(|entry| entry.as_u64())
            .unwrap_or(input + output);
        return Some(TokenUsage {
            prompt_tokens: input,
            completion_tokens: output,
            total_tokens,
            cache_creation_input_tokens: value
                .get("cache_creation_input_tokens")
                .and_then(|entry| entry.as_u64()),
            cache_read_input_tokens: value
                .get("cache_read_input_tokens")
                .and_then(|entry| entry.as_u64())
                .or_else(|| {
                    value
                        .get("input_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|entry| entry.as_u64())
                }),
        });
    }

    if let (Some(input), Some(output)) = (
        value.get("input_tokens").and_then(|entry| entry.as_u64()),
        value.get("output_tokens").and_then(|entry| entry.as_u64()),
    ) {
        let total_tokens = value
            .get("total_tokens")
            .and_then(|entry| entry.as_u64())
            .unwrap_or(input + output);
        return Some(TokenUsage {
            prompt_tokens: input,
            completion_tokens: output,
            total_tokens,
            cache_creation_input_tokens: value
                .get("cache_creation_input_tokens")
                .and_then(|entry| entry.as_u64()),
            cache_read_input_tokens: value
                .get("cache_read_input_tokens")
                .and_then(|entry| entry.as_u64())
                .or_else(|| {
                    value
                        .get("input_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|entry| entry.as_u64())
                }),
        });
    }

    if let (Some(prompt), Some(completion)) = (
        value.get("prompt_tokens").and_then(|entry| entry.as_u64()),
        value
            .get("completion_tokens")
            .and_then(|entry| entry.as_u64()),
    ) {
        let total_tokens = value
            .get("total_tokens")
            .and_then(|entry| entry.as_u64())
            .unwrap_or(prompt + completion);
        return Some(TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens,
            cache_creation_input_tokens: value
                .get("cache_creation_input_tokens")
                .and_then(|entry| entry.as_u64()),
            cache_read_input_tokens: value
                .get("cache_read_input_tokens")
                .and_then(|entry| entry.as_u64())
                .or_else(|| {
                    value
                        .get("prompt_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|entry| entry.as_u64())
                }),
        });
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::sync::{Arc, Mutex};

    type MetricsCap = Arc<Mutex<Option<(StreamMetrics, bool)>>>;

    fn make_bytes_stream(
        chunks: Vec<Result<Bytes, rquest::Error>>,
    ) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
        futures::stream::iter(chunks)
    }

    fn capture_callback() -> (
        MetricsCap,
        impl FnOnce(StreamMetrics, bool) + Send + 'static,
    ) {
        let cap: MetricsCap = Arc::new(Mutex::new(None));
        let cap2 = Arc::clone(&cap);
        let cb = move |m: StreamMetrics, success: bool| {
            *cap2.lock().unwrap() = Some((m, success));
        };
        (cap, cb)
    }

    // ── basic metrics ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn tracks_chunks_and_bytes() {
        let chunks = vec![Ok(Bytes::from("hello")), Ok(Bytes::from(" world"))];
        let (cap, cb) = capture_callback();
        let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

        while stream.next().await.is_some() {}

        let (m, success) = cap.lock().unwrap().clone().unwrap();
        assert_eq!(m.chunk_count, 2);
        assert_eq!(m.total_bytes, 11); // "hello" (5) + " world" (6)
        assert!(success);
    }

    #[tokio::test]
    async fn records_first_token_latency() {
        let chunks = vec![Ok(Bytes::from("first"))];
        let (cap, cb) = capture_callback();
        let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

        while stream.next().await.is_some() {}

        let (m, _) = cap.lock().unwrap().clone().unwrap();
        assert!(m.first_token_latency_ms.is_some());
    }

    #[tokio::test]
    async fn started_at_includes_elapsed_before_stream_construction() {
        let started_at = Instant::now();
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let chunks = vec![Ok(Bytes::from("first"))];
        let (cap, cb) = capture_callback();
        let mut stream =
            TrackedStream::new_with_started_at(make_bytes_stream(chunks), started_at, cb);

        while stream.next().await.is_some() {}

        let (metrics, _) = cap.lock().unwrap().clone().unwrap();
        assert!(metrics.first_token_latency_ms.unwrap_or_default() >= 20);
        assert!(metrics.total_duration_ms >= 20);
    }

    #[tokio::test]
    async fn no_first_token_latency_when_empty_stream() {
        let chunks: Vec<Result<Bytes, rquest::Error>> = vec![];
        let (cap, cb) = capture_callback();
        let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

        while stream.next().await.is_some() {}

        let (m, _) = cap.lock().unwrap().clone().unwrap();
        assert_eq!(m.first_token_latency_ms, None);
        assert_eq!(m.chunk_count, 0);
    }

    #[tokio::test]
    async fn callback_called_with_success_true_on_clean_end() {
        let chunks = vec![Ok(Bytes::from("data"))];
        let (cap, cb) = capture_callback();
        let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

        while stream.next().await.is_some() {}

        let (_, success) = cap.lock().unwrap().clone().unwrap();
        assert!(success);
    }

    // ── Drop behaviour ────────────────────────────────────────────────────

    #[tokio::test]
    async fn callback_fires_on_drop_with_false() {
        let chunks = vec![
            Ok(Bytes::from("chunk1")),
            Ok(Bytes::from("chunk2")),
            Ok(Bytes::from("chunk3")),
        ];
        let (cap, cb) = capture_callback();

        {
            let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);
            // Read only the first chunk then drop.
            let _ = stream.next().await;
            // stream dropped here
        }

        let result = cap.lock().unwrap().clone();
        assert!(result.is_some(), "callback should have fired on drop");
        let (m, success) = result.unwrap();
        assert!(!success, "success should be false when dropped early");
        assert_eq!(m.chunk_count, 1);
    }

    // ── callback fires exactly once ────────────────────────────────────────

    #[tokio::test]
    async fn callback_fires_only_once() {
        let fire_count = Arc::new(Mutex::new(0u32));
        let fc2 = Arc::clone(&fire_count);

        let chunks = vec![Ok(Bytes::from("x"))];
        let stream = TrackedStream::new(make_bytes_stream(chunks), move |_, _| {
            *fc2.lock().unwrap() += 1;
        });

        // Fully consume then drop.
        let mut pinned = Box::pin(stream);
        while pinned.next().await.is_some() {}
        drop(pinned);

        assert_eq!(*fire_count.lock().unwrap(), 1);
    }

    // ── metrics snapshot ──────────────────────────────────────────────────

    #[test]
    fn metrics_snapshot_reflects_no_data() {
        let stream = TrackedStream::new(make_bytes_stream(vec![]), |_, _| {});
        let m = stream.metrics();
        assert_eq!(m.chunk_count, 0);
        assert_eq!(m.total_bytes, 0);
        assert_eq!(m.first_token_latency_ms, None);
    }

    #[tokio::test]
    async fn tap_sse_usage_reads_openai_final_usage_chunk() {
        let chunks = vec![
            Ok(Bytes::from_static(
                br#"data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18}}

"#,
            )),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ];

        let (mut stream, usage) = tap_sse_usage(make_bytes_stream(chunks));
        while stream.next().await.is_some() {}

        let usage = snapshot_tapped_usage(&usage).unwrap();
        assert_eq!(usage.prompt_tokens, 11);
        assert_eq!(usage.completion_tokens, 7);
        assert_eq!(usage.total_tokens, 18);
    }

    #[tokio::test]
    async fn tap_sse_usage_combines_anthropic_message_start_and_delta() {
        let chunks = vec![
            Ok(Bytes::from_static(
                br#"event: message_start
data: {"type":"message_start","message":{"usage":{"input_tokens":13,"output_tokens":0}}}

"#,
            )),
            Ok(Bytes::from_static(
                br#"event: message_delta
data: {"type":"message_delta","usage":{"output_tokens":9,"cache_read_input_tokens":4}}

"#,
            )),
        ];

        let (mut stream, usage) = tap_sse_usage(make_bytes_stream(chunks));
        while stream.next().await.is_some() {}

        let usage = snapshot_tapped_usage(&usage).unwrap();
        assert_eq!(usage.prompt_tokens, 13);
        assert_eq!(usage.completion_tokens, 9);
        assert_eq!(usage.total_tokens, 22);
        assert_eq!(usage.cache_read_input_tokens, Some(4));
    }

    #[tokio::test]
    async fn tap_sse_usage_reads_nested_responses_completed_usage() {
        let chunks = vec![Ok(Bytes::from_static(
            br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_1","usage":{"input_tokens":17,"output_tokens":9,"total_tokens":26,"input_tokens_details":{"cached_tokens":5}}}}

"#,
        ))];

        let (mut stream, usage) = tap_sse_usage(make_bytes_stream(chunks));
        while stream.next().await.is_some() {}

        let usage = snapshot_tapped_usage(&usage).unwrap();
        assert_eq!(usage.prompt_tokens, 17);
        assert_eq!(usage.completion_tokens, 9);
        assert_eq!(usage.total_tokens, 26);
        assert_eq!(usage.cache_read_input_tokens, Some(5));
    }

    #[tokio::test]
    async fn tap_sse_completion_semantics_reads_openai_finish_reason() {
        let chunks = vec![
            Ok(Bytes::from_static(
                br#"data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}

"#,
            )),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ];

        let (mut stream, semantics) = tap_sse_completion_semantics(make_bytes_stream(chunks));
        while stream.next().await.is_some() {}

        assert_eq!(
            snapshot_tapped_completion_semantics(&semantics).as_deref(),
            Some("tool_calls")
        );
    }

    #[tokio::test]
    async fn tap_sse_completion_semantics_reads_anthropic_stop_reason() {
        let chunks = vec![Ok(Bytes::from_static(
            br#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"}}

"#,
        ))];

        let (mut stream, semantics) = tap_sse_completion_semantics(make_bytes_stream(chunks));
        while stream.next().await.is_some() {}

        assert_eq!(
            snapshot_tapped_completion_semantics(&semantics).as_deref(),
            Some("tool_calls")
        );
    }

    #[tokio::test]
    async fn tap_sse_completion_semantics_reads_responses_completed_status() {
        let chunks = vec![Ok(Bytes::from_static(
            br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_1","status":"completed","output":[{"type":"function_call","call_id":"call_1","name":"weather","arguments":"{}"}]}}

"#,
        ))];

        let (mut stream, semantics) = tap_sse_completion_semantics(make_bytes_stream(chunks));
        while stream.next().await.is_some() {}

        assert_eq!(
            snapshot_tapped_completion_semantics(&semantics).as_deref(),
            Some("tool_calls")
        );
    }
}
