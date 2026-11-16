//! Streaming completion semantics without changing forwarded bytes.
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

use super::observation::{BoundaryRule, ObservationLine, ObservationLines};
use super::ObservationScope;

struct CompletionSemanticsTappedStream<E> {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
    capture: SseCompletionSemanticsCapture,
}

#[derive(Debug)]
struct SseCompletionSemanticsCapture {
    scope: ObservationScope,
    latest_completion_semantics: Arc<Mutex<Option<String>>>,
    lines: ObservationLines,
    parser: SseParseState,
}

pub fn tap_sse_completion_semantics(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    Arc<Mutex<Option<String>>>,
) {
    tap_sse_completion_semantics_with_error(inner)
}

/// 完成语义快照与错误类型解耦，错误不会被解释成正常完成事件。
pub fn tap_sse_completion_semantics_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    Arc<Mutex<Option<String>>>,
) {
    tap_completion_mode(inner, ObservationScope::Generic)
}

/// Do not interpret opaque native extension fields as completion control data.
pub(crate) fn tap_messages_completion_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    Arc<Mutex<Option<String>>>,
) {
    tap_completion_mode(inner, ObservationScope::Messages)
}

/// Incomplete Responses retain their specific reason, even when output contains a tool call.
pub(crate) fn tap_commandcode_responses_completion_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> (
    impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    Arc<Mutex<Option<String>>>,
) {
    tap_completion_mode(inner, ObservationScope::CommandCodeResponses)
}

fn tap_completion_mode<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    scope: ObservationScope,
) -> (
    impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    Arc<Mutex<Option<String>>>,
) {
    let latest_completion_semantics = Arc::new(Mutex::new(None));
    let mut capture = SseCompletionSemanticsCapture::new(Arc::clone(&latest_completion_semantics));
    capture.scope = scope;
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

impl<E> Stream for CompletionSemanticsTappedStream<E> {
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

impl SseCompletionSemanticsCapture {
    fn new(latest_completion_semantics: Arc<Mutex<Option<String>>>) -> Self {
        Self {
            scope: ObservationScope::Generic,
            latest_completion_semantics,
            lines: ObservationLines::default(),
            parser: SseParseState::new(),
        }
    }

    fn feed(&mut self, chunk: &Bytes) {
        let mut lines = std::mem::take(&mut self.lines);
        lines.feed(
            chunk,
            BoundaryRule::TrimCarriageReturns,
            |line| match line {
                ObservationLine::Text(line) => {
                    if let Some(frame) = parse_sse_line(line, &mut self.parser) {
                        self.process_frame(frame.event_name.as_deref(), &frame.data);
                    }
                }
                ObservationLine::Reset => self.parser = SseParseState::new(),
            },
        );
        self.lines = lines;
    }

    fn process_frame(&mut self, event_name: Option<&str>, data: &str) {
        if data.is_empty() || data == "[DONE]" {
            return;
        }

        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return;
        };

        let semantics = if self.scope == ObservationScope::CommandCodeResponses {
            matches!(
                event_name,
                Some("response.completed" | "response.incomplete")
            )
            .then(|| {
                crate::protocol::commandcode::native_json::observe(
                    &value["response"],
                    crate::protocol::canonical::EndpointKind::Responses,
                )
                .1
            })
        } else if self.scope == ObservationScope::Messages {
            (event_name == Some("message_delta"))
                .then(|| value["delta"]["stop_reason"].as_str())
                .flatten()
                .and_then(|reason| match reason {
                    "refusal" => Some("content_filter"),
                    "model_context_window_exceeded" => Some("length"),
                    _ => map_completion_reason(reason),
                })
        } else {
            extract_completion_semantics(event_name, &value)
        };
        let Some(semantics) = semantics else {
            return;
        };

        if let Ok(mut latest_completion_semantics) = self.latest_completion_semantics.lock() {
            *latest_completion_semantics = Some(semantics.to_string());
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

#[test]
fn commandcode_messages_completion_ignores_extension_control_fields() {
    for (reason, expected) in [
        ("end_turn", "stop"),
        ("pause_turn", "other_provider_reason"),
        ("refusal", "content_filter"),
        ("model_context_window_exceeded", "length"),
    ] {
        let handle = Arc::new(Mutex::new(None));
        let mut capture = SseCompletionSemanticsCapture::new(handle.clone());
        capture.scope = ObservationScope::Messages;
        capture.process_frame(
            Some("message_delta"),
            &serde_json::json!({"delta":{"stop_reason":reason},"status":"incomplete"}).to_string(),
        );
        capture.process_frame(
            Some("provider_trace"),
            r#"{"status":"completed","output":[{"type":"function_call"}]}"#,
        );
        assert_eq!(handle.lock().unwrap().as_deref(), Some(expected));
    }
}
