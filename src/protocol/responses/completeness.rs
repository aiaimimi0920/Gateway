//! Transparent, lazy completion validation before passthrough/translation can invent success.
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;

use super::terminal::Completion;
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::ProtocolStreamError;

pub fn require_responses_completion<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    CompletionStream {
        inner: Some(Box::pin(inner)),
        decoder: BoundedSseDecoder::new(MAX_TRANSLATED_SSE_FRAME_BYTES),
        completion: Completion::default(),
        pending_error: None,
    }
}

struct CompletionStream<E> {
    inner: Option<Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>>,
    decoder: BoundedSseDecoder,
    completion: Completion,
    pending_error: Option<ProtocolStreamError>,
}

impl<E> CompletionStream<E> {
    fn observe(
        &mut self,
        frame: crate::protocol::sse_parse::SseFrame,
    ) -> Result<(), ProtocolStreamError> {
        self.completion
            .observe(&frame)
            .map(|_| ())
            .map_err(|error| error.protocol())
    }

    fn clear(&mut self) {
        // Release the transport and bounded parser before the outer terminal callback runs.
        self.inner = None;
        self.decoder.clear();
    }
}

impl<E: From<ProtocolStreamError>> Stream for CompletionStream<E> {
    type Item = Result<Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(error) = self.pending_error.take() {
            return Poll::Ready(Some(Err(error.into())));
        }
        let Some(inner) = self.inner.as_mut() else {
            return Poll::Ready(None);
        };
        match inner.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(bytes))) => {
                // Bytes::clone retains the original allocation; no whole-stream accumulation.
                self.decoder.push_chunk(bytes.clone());
                loop {
                    let error = match self.decoder.decode_next() {
                        DecodeStep::Frame(frame) => self.observe(frame).err(),
                        DecodeStep::Error(error) => Some(error),
                        DecodeStep::NeedInput => break,
                    };
                    if let Some(error) = error {
                        self.clear();
                        if error.is_upstream_failure() {
                            // Preserve the upstream failure event on the wire before marking
                            // accounting failed. Never synthesize a successful completion.
                            self.pending_error = Some(error);
                            return Poll::Ready(Some(Ok(bytes)));
                        }
                        return Poll::Ready(Some(Err(error.into())));
                    }
                }
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(error))) => {
                self.clear();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                let result = self
                    .decoder
                    .flush_pending_frame()
                    .map_or(Ok(()), |frame| self.observe(frame))
                    .and_then(|_| self.completion.finish().map_err(|error| error.protocol()));
                self.clear();
                Poll::Ready(result.err().map(|error| Err(error.into())))
            }
        }
    }
}
