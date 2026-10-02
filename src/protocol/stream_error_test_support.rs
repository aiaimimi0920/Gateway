//! Poll/drop instrumentation shared by the typed stream boundary contracts.
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::{Stream, StreamExt};

/// 由真实响应 body 路径产生客户端错误，不依赖客户端私有构造器。
pub(crate) async fn http_body_error(message: &str) -> rquest::Error {
    let body = futures::stream::iter([Err::<Bytes, _>(std::io::Error::other(message.to_owned()))]);
    let response =
        rquest::Response::from(axum::http::Response::new(rquest::Body::wrap_stream(body)));
    let mut stream = Box::pin(response.bytes_stream());
    stream.next().await.unwrap().unwrap_err()
}

/// 客户端的顶层 Display 可变化，但流式原始失败原因不能从 source chain 消失。
pub(crate) fn source_messages(error: &(dyn std::error::Error + 'static)) -> String {
    let mut current = Some(error);
    let mut messages = Vec::new();
    for _ in 0..16 {
        let Some(error) = current else { break };
        messages.push(error.to_string());
        current = error.source();
    }
    messages.join(": ")
}

#[derive(Default)]
pub(crate) struct ProbeState {
    pub(crate) polls: AtomicUsize,
    pub(crate) drops: AtomicUsize,
}

pub(crate) struct Probe<E> {
    chunks: VecDeque<Result<Bytes, E>>,
    pending_at_end: bool,
    state: Arc<ProbeState>,
}

impl<E: Unpin> Stream for Probe<E> {
    type Item = Result<Bytes, E>;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.state.polls.fetch_add(1, Ordering::SeqCst);
        if let Some(chunk) = this.chunks.pop_front() {
            Poll::Ready(Some(chunk))
        } else if this.pending_at_end {
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
}

impl<E> Drop for Probe<E> {
    fn drop(&mut self) {
        self.state.drops.fetch_add(1, Ordering::SeqCst);
    }
}

pub(crate) fn probe<E>(
    chunks: Vec<Result<Bytes, E>>,
    pending_at_end: bool,
) -> (Probe<E>, Arc<ProbeState>) {
    let state = Arc::new(ProbeState::default());
    (
        Probe {
            chunks: chunks.into(),
            pending_at_end,
            state: Arc::clone(&state),
        },
        state,
    )
}
