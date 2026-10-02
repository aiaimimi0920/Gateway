// ---------------------------------------------------------------------------
// upstream/stream.rs — TrackedStream
//
// A pinned Stream wrapper that records first-token latency, chunk count,
// byte count, and total duration, then fires a callback when the stream
// terminates (either naturally or via Drop).
// ---------------------------------------------------------------------------

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use bytes::Bytes;
use futures::Stream;

mod archive;
mod completion;
mod observation;
mod usage;

pub use archive::{
    snapshot_tapped_archive, tap_stream_archive, tap_stream_archive_with_error,
    StreamArchiveHandle, StreamArchiveSnapshot,
};
pub use completion::{
    snapshot_tapped_completion_semantics, tap_sse_completion_semantics,
    tap_sse_completion_semantics_with_error,
};
pub use usage::{snapshot_tapped_usage, tap_sse_usage, tap_sse_usage_with_error};

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
pub struct TrackedStream<E = rquest::Error> {
    inner: Option<Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>>,
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
        Self::new_with_started_at_and_error(inner, started_at, on_complete)
    }
}

impl<E> TrackedStream<E> {
    /// 保留调用者的错误对象；旧构造入口仍固定客户端类型，避免破坏空流的类型推断。
    pub fn new_with_error(
        inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
        on_complete: impl FnOnce(StreamMetrics, bool) + Send + 'static,
    ) -> Self {
        Self::new_with_started_at_and_error(inner, Instant::now(), on_complete)
    }

    pub fn new_with_started_at_and_error(
        inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
        started_at: Instant,
        on_complete: impl FnOnce(StreamMetrics, bool) + Send + 'static,
    ) -> Self {
        Self {
            inner: Some(Box::pin(inner)),
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
        // Release transport and captured state before reporting terminal completion.
        self.inner = None;
        if let Some(cb) = self.on_complete.take() {
            cb(self.metrics(), success);
        }
    }
}

// ---------------------------------------------------------------------------
// Stream impl
// ---------------------------------------------------------------------------

impl<E> Stream for TrackedStream<E> {
    type Item = Result<Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Some(inner) = self.inner.as_mut() else {
            return Poll::Ready(None);
        };

        match inner.as_mut().poll_next(cx) {
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

impl<E> Drop for TrackedStream<E> {
    fn drop(&mut self) {
        // Ensure callback fires even if the stream is dropped before exhaustion
        // (e.g., the client disconnected).
        self.fire_callback(false);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;

#[cfg(test)]
mod typed_error_tests;
