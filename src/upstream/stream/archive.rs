//! Bounded response archive capture and shared snapshots.
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;

struct ArchiveTappedStream<E> {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
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
struct StreamArchiveCapture {
    state: Arc<Mutex<StreamArchiveCaptureState>>,
}

#[derive(Debug)]
struct StreamArchiveCaptureState {
    bytes: Vec<u8>,
    truncated: bool,
    max_bytes: usize,
}

pub fn tap_stream_archive(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    max_bytes: usize,
) -> (
    impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    StreamArchiveHandle,
) {
    tap_stream_archive_with_error(inner, max_bytes)
}

/// 归档只观察成功字节，不重新包装或复制流错误。
pub fn tap_stream_archive_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    max_bytes: usize,
) -> (
    impl Stream<Item = Result<Bytes, E>> + Send + 'static,
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

impl<E> Stream for ArchiveTappedStream<E> {
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

impl StreamArchiveCapture {
    fn feed(&mut self, chunk: &Bytes) {
        if chunk.is_empty() {
            return;
        }
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
