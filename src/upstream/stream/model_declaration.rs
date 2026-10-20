//! Observe original SSE envelopes, before any adapter can synthesize model names.
//! Bytes/errors remain untouched; EOF, error and cancellation each publish once.
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

use super::observation::{BoundaryRule, ObservationLine, ObservationLines};
use crate::metrics::model_evidence::ModelObservationMeta;
use crate::metrics::request::GatewayMetrics;
use crate::protocol::model_declaration::{ModelDeclaration, ModelDeclarationSource};

const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_OBSERVED_BYTES: usize = 8 * 1024 * 1024;

pub(crate) fn tap_model_declaration<S, E>(
    stream: S,
    metadata: ModelObservationMeta,
    metrics: Arc<GatewayMetrics>,
    original_http: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
{
    ModelDeclarationStream {
        inner: Box::pin(stream),
        lines: ObservationLines::new(MAX_FRAME_BYTES, 1024),
        state: ObservationState {
            disabled: (!original_http).then_some("synthetic_or_unobserved"),
            ..Default::default()
        },
        metadata: Some(metadata),
        metrics,
    }
}

struct ModelDeclarationStream<E> {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
    lines: ObservationLines,
    state: ObservationState,
    metadata: Option<ModelObservationMeta>,
    metrics: Arc<GatewayMetrics>,
}

#[derive(Default)]
struct ObservationState {
    data: Vec<u8>,
    declaration: Option<ModelDeclaration>,
    disabled: Option<&'static str>,
    total: usize,
}

impl ObservationState {
    fn disable(&mut self, reason: &'static str) {
        self.disabled = Some(reason);
        self.declaration = None;
        self.data = Vec::new();
    }

    fn on_line(&mut self, line: ObservationLine<'_>) {
        if self.disabled.is_some() {
            return;
        }
        match line {
            ObservationLine::Reset => self.disable("observation_limit"),
            ObservationLine::Text("") => {
                if self.data.is_empty() || self.data == b"[DONE]" {
                    self.data.clear();
                    return;
                }
                match serde_json::from_slice::<Value>(&self.data) {
                    Ok(body) => self.observe(&body),
                    Err(_) => self.disable("invalid_envelope"),
                }
                self.data.clear();
            }
            ObservationLine::Text(line) => {
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.strip_prefix(' ').unwrap_or(data);
                    if !self.data.is_empty() {
                        self.data.push(b'\n');
                    }
                    self.data.extend_from_slice(data.as_bytes());
                }
            }
        }
    }

    fn observe(&mut self, body: &Value) {
        // Only known response envelopes, never recursive searches through user content.
        let declaration = if body.get("choices").is_some_and(Value::is_array) {
            body.get("model")
                .map(|v| (v, ModelDeclarationSource::Model))
        } else {
            match body.get("type").and_then(Value::as_str) {
                Some("message_start") => body
                    .pointer("/message/model")
                    .map(|v| (v, ModelDeclarationSource::MessageModel)),
                Some(
                    "response.created"
                    | "response.in_progress"
                    | "response.completed"
                    | "response.incomplete"
                    | "response.failed",
                ) => body
                    .pointer("/response/model")
                    .map(|v| (v, ModelDeclarationSource::ResponseModel)),
                _ => None,
            }
        };
        let Some((value, source)) = declaration else {
            return;
        };
        let Some(declaration) = ModelDeclaration::from_value(value, source) else {
            self.disable("invalid_declaration");
            return;
        };
        if self
            .declaration
            .as_ref()
            .is_some_and(|old| old.value != declaration.value)
        {
            self.disable("conflicting_declarations");
        } else {
            self.declaration = Some(declaration);
        }
    }
}

impl<E> ModelDeclarationStream<E> {
    fn finish(&mut self, outcome: &'static str) {
        if let Some(metadata) = self.metadata.take() {
            let status = self
                .state
                .disabled
                .unwrap_or(if self.state.declaration.is_some() {
                    "declared"
                } else {
                    "missing_or_unobserved"
                });
            self.metrics.observe_model_declaration(metadata.finish(
                self.state.declaration.as_ref(),
                status,
                outcome,
            ));
        }
    }
}

impl<E> Stream for ModelDeclarationStream<E> {
    type Item = Result<Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        let result = this.inner.as_mut().poll_next(cx);
        match &result {
            Poll::Ready(Some(Ok(chunk)))
                if this.metadata.is_some() && this.state.disabled.is_none() =>
            {
                this.state.total = this.state.total.saturating_add(chunk.len());
                if this.state.total > MAX_OBSERVED_BYTES {
                    this.state.disable("observation_limit");
                } else {
                    let state = &mut this.state;
                    this.lines
                        .feed(chunk, BoundaryRule::Empty, |line| state.on_line(line));
                }
            }
            Poll::Ready(Some(Err(_))) => this.finish("transport_error"),
            Poll::Ready(None) => this.finish("eof"),
            _ => {}
        }
        result
    }
}

impl<E> Drop for ModelDeclarationStream<E> {
    fn drop(&mut self) {
        // A translator may drop its source at a valid terminal; not proof of client cancellation.
        self.finish("source_dropped");
    }
}

#[cfg(test)]
mod tests;
