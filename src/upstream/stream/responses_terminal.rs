//! Observe response-level completion without consuming ahead or changing forwarded bytes.
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;

use super::observation::{BoundaryRule, ObservationLine, ObservationLines};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

pub(super) fn tap<E: Send + 'static>(
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
) -> (impl Stream<Item = Result<Bytes, E>> + Send, Arc<AtomicBool>) {
    let completed = Arc::new(AtomicBool::new(false));
    let mut capture = Capture {
        completed: Arc::clone(&completed),
        lines: ObservationLines::default(),
        parser: SseParseState::new(),
        untrusted: false,
    };
    let stream = inner.map(move |result| {
        match &result {
            Ok(bytes) => capture.feed(bytes),
            Err(_) => capture.reject(),
        }
        result
    });
    (stream, completed)
}

struct Capture {
    completed: Arc<AtomicBool>,
    lines: ObservationLines,
    parser: SseParseState,
    untrusted: bool,
}

impl Capture {
    fn reject(&mut self) {
        self.untrusted = true;
        self.completed.store(false, Ordering::Release);
    }

    fn feed(&mut self, bytes: &Bytes) {
        let mut lines = std::mem::take(&mut self.lines);
        lines.feed(
            bytes,
            BoundaryRule::TrimCarriageReturns,
            |line| match line {
                ObservationLine::Reset => {
                    // Oversize/allocation failures cannot hide a conflicting terminal/error.
                    self.parser = SseParseState::new();
                    self.reject();
                }
                ObservationLine::Text(line) => {
                    if let Some(frame) = parse_sse_line(line, &mut self.parser) {
                        self.observe(frame.event_name.as_deref(), &frame.data);
                    }
                }
            },
        );
        self.lines = lines;
    }

    fn observe(&mut self, event: Option<&str>, data: &str) {
        if self.untrusted || data.is_empty() || data == "[DONE]" {
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            self.reject();
            return;
        };
        let kind = value.get("type").and_then(Value::as_str);
        if event.is_some() && kind.is_some() && event != kind {
            self.reject();
            return;
        }
        let kind = event.or(kind).unwrap_or_default();
        if matches!(kind, "error" | "response.failed" | "response.cancelled")
            || value.get("error").is_some_and(|v| !v.is_null())
            || value
                .pointer("/response/error")
                .is_some_and(|v| !v.is_null())
        {
            self.reject();
        } else if matches!(kind, "response.completed" | "response.incomplete") {
            let expected = if kind == "response.completed" {
                "completed"
            } else {
                "incomplete"
            };
            if value.pointer("/response/status").and_then(Value::as_str) == Some(expected) {
                self.completed.store(true, Ordering::Release);
            } else {
                self.reject();
            }
        } else if matches!(kind, "response.created" | "response.in_progress") {
            self.completed.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
#[path = "responses_terminal_capture_tests.rs"]
mod tests;
