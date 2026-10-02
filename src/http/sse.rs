// ---------------------------------------------------------------------------
// http/sse.rs — Convert a TrackedStream into an axum SSE HTTP response
// ---------------------------------------------------------------------------

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures::{Future, StreamExt};
use tokio::time::{Instant, Sleep};

use crate::protocol::canonical::EndpointKind;
use crate::upstream::stream::TrackedStream;

const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);
const SSE_KEEPALIVE_FRAME: &[u8] = b": keepalive\n\n";

/// Convert a [`TrackedStream`] into a streaming HTTP response.
///
/// The upstream byte stream is forwarded verbatim as `text/event-stream`.
/// This is correct because the upstream providers already emit fully-formed
/// SSE frames; we act as a transparent proxy for the byte stream.
pub fn into_sse_response<E>(
    stream: TrackedStream<E>,
    _endpoint_kind: EndpointKind,
) -> impl IntoResponse
where
    E: std::error::Error + Send + Sync + 'static,
{
    let stream = wrap_with_keepalive(stream, SSE_KEEPALIVE_INTERVAL);
    // 只在 HTTP Body 边界装箱，保留领域错误及其原始 source 链。
    let mapped =
        stream.map(|r| r.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>));

    let body = Body::from_stream(mapped);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .expect("valid SSE response builder")
}

fn wrap_with_keepalive<E>(stream: TrackedStream<E>, interval: Duration) -> KeepAliveStream<E> {
    KeepAliveStream::new(stream, interval)
}

struct KeepAliveStream<E> {
    inner: TrackedStream<E>,
    interval: Duration,
    next_ping: Pin<Box<Sleep>>,
}

impl<E> KeepAliveStream<E> {
    fn new(inner: TrackedStream<E>, interval: Duration) -> Self {
        Self {
            inner,
            interval,
            next_ping: Box::pin(tokio::time::sleep(interval)),
        }
    }

    fn reset_ping(&mut self) {
        self.next_ping
            .as_mut()
            .reset(Instant::now() + self.interval);
    }
}

impl<E> futures::Stream for KeepAliveStream<E> {
    type Item = Result<Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(item)) => {
                self.reset_ping();
                return Poll::Ready(Some(item));
            }
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        if self.next_ping.as_mut().poll(cx).is_ready() {
            self.reset_ping();
            return Poll::Ready(Some(Ok(Bytes::from_static(SSE_KEEPALIVE_FRAME))));
        }

        Poll::Pending
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "sse_error_tests.rs"]
mod error_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::EndpointKind;
    use crate::upstream::stream::TrackedStream;
    use bytes::Bytes;
    use futures::stream;

    fn make_tracked_stream(chunks: Vec<&'static str>) -> TrackedStream {
        let byte_chunks: Vec<Result<Bytes, rquest::Error>> = chunks
            .iter()
            .map(|s| Ok(Bytes::from_static(s.as_bytes())))
            .collect();
        TrackedStream::new(stream::iter(byte_chunks), |_, _| {})
    }

    #[tokio::test]
    async fn into_sse_response_returns_200() {
        let stream = make_tracked_stream(vec!["data: hello\n\n"]);
        let response = into_sse_response(stream, EndpointKind::ChatCompletions).into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn into_sse_response_content_type_is_text_event_stream() {
        let stream = make_tracked_stream(vec![]);
        let response = into_sse_response(stream, EndpointKind::Messages).into_response();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.contains("text/event-stream"),
            "expected text/event-stream, got: {content_type}"
        );
    }

    #[tokio::test]
    async fn into_sse_response_cache_control_no_cache() {
        let stream = make_tracked_stream(vec![]);
        let response = into_sse_response(stream, EndpointKind::ChatCompletions).into_response();
        let cc = response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(cc, "no-cache");
    }
}
