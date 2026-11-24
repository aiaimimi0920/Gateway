//! Lazy, bounded Chat SSE guard. Only validated EOF can release finish/usage/DONE.
use super::chat_stream_state::Admission;
use crate::protocol::{
    stream_decode::{BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::Stream;
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) fn require_completion<S, E>(
    stream: S,
    error: fn() -> ProtocolStreamError,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    guard(stream, error, false, None)
}
/// Project the public model on validated Chat frames, without touching nested payloads.
pub(crate) fn require_completion_with_reply_model<S, E>(
    stream: S,
    error: fn() -> ProtocolStreamError,
    reply_model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    guard(stream, error, false, Some(reply_model))
}
/// Some Chat products document finish_reason or DONE as the terminal signal.
pub(crate) fn require_terminal<S, E>(
    stream: S,
    error: fn() -> ProtocolStreamError,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    guard(stream, error, true, None)
}
fn guard<S, E>(
    stream: S,
    error: fn() -> ProtocolStreamError,
    either_terminal: bool,
    reply_model: Option<String>,
) -> Guard<E>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
{
    Guard {
        inner: Some(Box::pin(stream)),
        decoder: BoundedSseDecoder::new_strict(MAX_TRANSLATED_SSE_FRAME_BYTES),
        admission: Admission::new(either_terminal, reply_model),
        held: VecDeque::new(),
        held_bytes: 0,
        eof: false,
        limit: MAX_TRANSLATED_SSE_FRAME_BYTES,
        error,
    }
}
struct Guard<E> {
    inner: Option<Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>>,
    decoder: BoundedSseDecoder,
    admission: Admission,
    held: VecDeque<Bytes>,
    held_bytes: usize,
    eof: bool,
    limit: usize,
    error: fn() -> ProtocolStreamError,
}
impl<E> Guard<E> {
    fn clear(&mut self) {
        self.inner = None;
        self.decoder.clear();
        self.held.clear();
        self.held_bytes = 0;
        self.eof = true;
    }
    fn retain(&mut self, bytes: Bytes) -> Result<(), ProtocolStreamError> {
        let size = self
            .held_bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(invalid)?;
        // Bound object overhead as well as bytes; typical workspace tails have two or three frames.
        if self.held.len() >= 256 {
            return Err(invalid());
        }
        self.held.try_reserve(1).map_err(|_| invalid())?;
        self.held_bytes = size;
        self.held.push_back(bytes);
        Ok(())
    }
}
impl<E: From<ProtocolStreamError>> Stream for Guard<E> {
    type Item = Result<Bytes, E>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.eof {
            return Poll::Ready(self.held.pop_front().map(Ok));
        }
        // Do not drain arbitrarily many terminal/ping frames in a single poll.
        for _ in 0..32 {
            match self.decoder.decode_next() {
                DecodeStep::Frame(frame) => {
                    let accepted = self.admission.accept(frame).and_then(|(live, held)| {
                        if let Some(bytes) = held {
                            self.retain(bytes)?;
                        }
                        Ok(live)
                    });
                    match accepted {
                        Ok(Some(bytes)) => return Poll::Ready(Some(Ok(bytes))),
                        Ok(None) => continue,
                        Err(_) => {
                            let error = (self.error)();
                            self.clear();
                            return Poll::Ready(Some(Err(error.into())));
                        }
                    }
                }
                DecodeStep::Error(_) => {
                    self.clear();
                    return Poll::Ready(Some(Err((self.error)().into())));
                }
                DecodeStep::NeedInput => {}
            }
            match self.inner.as_mut().unwrap().as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(bytes))) => self.decoder.push_chunk(bytes),
                Poll::Ready(Some(Err(error))) => {
                    self.clear();
                    return Poll::Ready(Some(Err(error)));
                }
                Poll::Ready(None) => {
                    let complete =
                        self.decoder.finish_strict().is_ok() && self.admission.complete();
                    self.inner = None;
                    self.decoder.clear();
                    self.eof = true;
                    if !complete {
                        self.clear();
                        return Poll::Ready(Some(Err((self.error)().into())));
                    }
                    if let Some(done) = self.admission.eof_terminal() {
                        if self.retain(done).is_err() {
                            self.clear();
                            return Poll::Ready(Some(Err((self.error)().into())));
                        }
                    }
                    return Poll::Ready(self.held.pop_front().map(Ok));
                }
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
pub(super) fn invalid() -> ProtocolStreamError {
    // Fixed diagnostics: neither JSON/parser details nor upstream error contents cross this boundary.
    ProtocolStreamError::invalid_data("invalid_chat_stream: incomplete or invalid Chat stream")
}

#[cfg(test)]
#[path = "../qwen/workspace_stream_tests.rs"]
mod tests;
