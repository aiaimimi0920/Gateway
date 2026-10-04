//! Exactly-once request lifetime: middleware future -> response body -> EOF/error/drop.
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};

use crate::metrics::{diagnostics::RequestTermination, request::GatewayMetrics};
use crate::state::AppState;

pub(super) struct RequestObservation {
    state: Arc<AppState>,
    metrics: Arc<GatewayMetrics>,
    started_at: Instant,
    status: u16,
    drain_rejected: bool,
    active: bool,
    completed: bool,
}

impl RequestObservation {
    pub fn new(state: Arc<AppState>, metrics: Arc<GatewayMetrics>, drain_rejected: bool) -> Self {
        let active = !drain_rejected;
        if active {
            state.lifecycle.begin_request();
            metrics.begin_request();
        }
        Self {
            state,
            metrics,
            started_at: Instant::now(),
            status: 0,
            drain_rejected,
            active,
            completed: false,
        }
    }

    fn finish(&mut self, terminal: RequestTermination) {
        if self.completed {
            return;
        }
        self.completed = true;
        if self.active {
            self.metrics.end_request();
            self.state.lifecycle.end_request();
        }
        let duration = u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.metrics
            .observe_request_terminal(self.status, duration, self.drain_rejected, terminal);
    }

    pub fn wrap(mut self, status: u16, body: Body, head: bool) -> Body {
        self.status = status;
        // Empty responses, HEAD and upgraded connections finish at the handshake.
        // Other bodies stay in-flight until the server consumes or drops them.
        if head || body.is_end_stream() {
            drop(body);
            self.finish(RequestTermination::Completed);
            return Body::empty();
        }
        Body::new(ObservedBody {
            inner: Some(body),
            observation: self,
        })
    }
}

impl Drop for RequestObservation {
    fn drop(&mut self) {
        self.finish(RequestTermination::Cancelled);
    }
}

struct ObservedBody {
    inner: Option<Body>,
    observation: RequestObservation,
}

impl ObservedBody {
    fn finish(&mut self, terminal: RequestTermination) {
        // Release source body/stream and its terminal callback before request accounting.
        self.inner = None;
        self.observation.finish(terminal);
    }
}

impl HttpBody for ObservedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let Some(inner) = self.inner.as_mut() else {
            return Poll::Ready(None);
        };
        let result = Pin::new(&mut *inner).poll_frame(cx);
        let terminal = match &result {
            Poll::Ready(Some(Err(_))) => Some(RequestTermination::Interrupted),
            Poll::Ready(None) => Some(RequestTermination::Completed),
            Poll::Ready(Some(Ok(_))) if inner.is_end_stream() => {
                Some(RequestTermination::Completed)
            }
            _ => None,
        };
        if let Some(terminal) = terminal {
            self.finish(terminal);
        }
        result
    }

    fn is_end_stream(&self) -> bool {
        self.inner.as_ref().is_none_or(HttpBody::is_end_stream)
    }
    fn size_hint(&self) -> SizeHint {
        self.inner
            .as_ref()
            .map(HttpBody::size_hint)
            .unwrap_or_default()
    }
}

impl Drop for ObservedBody {
    fn drop(&mut self) {
        self.finish(RequestTermination::Cancelled);
    }
}
