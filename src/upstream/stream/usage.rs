//! Streaming token-usage observation and provider usage normalization.
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

use crate::protocol::canonical::TokenUsage;

use super::observation::{BoundaryRule, ObservationLine, ObservationLines};

struct UsageTappedStream<E> {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
    capture: SseUsageCapture,
}

#[derive(Debug)]
struct SseUsageCapture {
    latest_usage: Arc<Mutex<Option<TokenUsage>>>,
    lines: ObservationLines,
    pending_event: Option<String>,
    pending_data_lines: Vec<String>,
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

pub fn tap_sse_usage(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    Arc<Mutex<Option<TokenUsage>>>,
) {
    tap_sse_usage_with_error(inner)
}

/// usage 观察仅处理成功字节，保留传输或协议错误的原始类型和对象。
pub fn tap_sse_usage_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, E>> + Send + 'static,
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

impl<E> Stream for UsageTappedStream<E> {
    type Item = Result<Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
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
            lines: ObservationLines::default(),
            pending_event: None,
            pending_data_lines: Vec::new(),
            prompt_tokens: None,
            completion_tokens: None,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }
    }

    fn feed(&mut self, chunk: &Bytes) {
        let mut lines = std::mem::take(&mut self.lines);
        lines.feed(chunk, BoundaryRule::Empty, |line| match line {
            ObservationLine::Text(line) => self.process_line(line),
            ObservationLine::Reset => {
                self.pending_event = None;
                self.pending_data_lines = Vec::new();
            }
        });
        self.lines = lines;
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

fn parse_usage_value(value: &Value) -> Option<TokenUsage> {
    if let Some(output) = value.get("output_tokens").and_then(|entry| entry.as_u64()) {
        let input = value
            .get("input_tokens")
            .and_then(|entry| entry.as_u64())
            .unwrap_or(0);
        let total_tokens = value
            .get("total_tokens")
            .and_then(|entry| entry.as_u64())
            .unwrap_or(input.saturating_add(output));
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
            .unwrap_or(prompt.saturating_add(completion));
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
