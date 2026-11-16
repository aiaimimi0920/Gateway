//! 在协议转换前验证 Messages 终态；延迟释放结束帧，避免错误尾部被转换成成功。
use crate::protocol::{
    sse_parse::format_sse_event,
    stream_decode::{BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::Stream;
use serde_json::Value;
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) fn require_completion<S, E>(stream: S) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    Guard {
        inner: Some(Box::pin(stream)),
        decoder: BoundedSseDecoder::new_strict(MAX_TRANSLATED_SSE_FRAME_BYTES),
        started: false,
        finishing: false,
        stopped: false,
        held: VecDeque::new(),
        held_bytes: 0,
        eof: false,
    }
}
struct Guard<E> {
    inner: Option<Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>>,
    decoder: BoundedSseDecoder,
    started: bool,
    finishing: bool,
    stopped: bool,
    held: VecDeque<Bytes>,
    held_bytes: usize,
    eof: bool,
}
impl<E> Guard<E> {
    fn clear(&mut self) {
        self.inner = None;
        self.decoder.clear();
        self.held.clear();
        self.held_bytes = 0;
        self.eof = true;
    }
    fn accept(
        &mut self,
        frame: crate::protocol::sse_parse::SseFrame,
    ) -> Result<Option<Bytes>, ProtocolStreamError> {
        if frame.data.is_empty() {
            return Ok(None);
        }
        let value: Value = serde_json::from_str(&frame.data).map_err(|_| invalid())?;
        let kind = value["type"].as_str().ok_or_else(invalid)?;
        if frame.event_name.as_deref().is_some_and(|name| name != kind)
            || kind == "error"
            || !value["error"].is_null()
        {
            return Err(invalid());
        }
        if kind == "ping" {
            return Ok(None);
        }
        if self.stopped {
            return Err(invalid());
        }
        match kind {
            "message_start"
                if !self.started
                    && value["message"]["type"] == "message"
                    && value["message"]["role"] == "assistant"
                    && value["message"]["content"].is_array() =>
            {
                self.started = true
            }
            "message_delta"
                if self.started
                    && !self.finishing
                    && value["delta"]["stop_reason"]
                        .as_str()
                        .is_some_and(|v| !v.is_empty()) =>
            {
                self.finishing = true
            }
            "message_stop" if self.finishing => self.stopped = true,
            "content_block_start" | "content_block_delta" | "content_block_stop"
                if self.started && !self.finishing => {}
            _ => return Err(invalid()),
        }
        let bytes = Bytes::from(format_sse_event(frame.event_name.as_deref(), &frame.data));
        if !self.finishing {
            return Ok(Some(bytes));
        }
        self.held_bytes = self
            .held_bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_TRANSLATED_SSE_FRAME_BYTES)
            .ok_or_else(invalid)?;
        self.held.push_back(bytes); // 仅 message_delta 和 message_stop，最多两帧。
        Ok(None)
    }
}
impl<E: From<ProtocolStreamError>> Stream for Guard<E> {
    type Item = Result<Bytes, E>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.eof {
            return Poll::Ready(self.held.pop_front().map(Ok));
        }
        for _ in 0..32 {
            match self.decoder.decode_next() {
                DecodeStep::Frame(frame) => match self.accept(frame) {
                    Ok(Some(bytes)) => return Poll::Ready(Some(Ok(bytes))),
                    Ok(None) => continue,
                    Err(error) => {
                        self.clear();
                        return Poll::Ready(Some(Err(error.into())));
                    }
                },
                DecodeStep::Error(_) => {
                    self.clear();
                    return Poll::Ready(Some(Err(invalid().into())));
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
                    if !self.stopped || self.decoder.finish_strict().is_err() {
                        self.clear();
                        return Poll::Ready(Some(Err(invalid().into())));
                    }
                    self.inner = None;
                    self.decoder.clear();
                    self.eof = true;
                    return Poll::Ready(self.held.pop_front().map(Ok));
                }
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
fn invalid() -> ProtocolStreamError {
    ProtocolStreamError::classified(
        "invalid_minimax_stream: incomplete or failed Messages stream",
        super::response_error(),
    )
}
