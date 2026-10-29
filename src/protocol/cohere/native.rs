//! Cohere v2 SSE owner, independent of optional Accio implementation features.
use crate::{
    error::GatewayError,
    protocol::{
        canonical::CanonicalRelayResponse,
        sse_parse::{parse_sse_line, SseParseState},
        stream_error::{ProtocolStreamError, StreamError},
    },
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::{collections::VecDeque, pin::Pin};
mod events;
mod response;
pub use response::unpack;

const FRAME_LIMIT: usize = 256 * 1024;
struct Decoder {
    buffer: Vec<u8>,
    frame: SseParseState,
    frame_bytes: usize,
    events: events::Events,
    outputs: VecDeque<Bytes>,
    output_bytes: usize,
}

impl Decoder {
    fn new(model: &str, collect: bool) -> Self {
        Self {
            buffer: Vec::new(),
            frame: SseParseState::new(),
            frame_bytes: 0,
            events: events::Events::new(model, collect),
            outputs: VecDeque::new(),
            output_bytes: 0,
        }
    }

    fn feed(&mut self, bytes: &[u8]) -> Result<(), GatewayError> {
        // Process one line at a time so arbitrary HTTP chunk boundaries cannot split UTF-8.
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            // A real terminal event owns completion, independent of HTTP chunk boundaries.
            if self.events.terminal {
                break;
            }
            self.frame_bytes = self.frame_bytes.saturating_add(part.len());
            if self.frame_bytes > FRAME_LIMIT {
                return Err(response::invalid());
            }
            self.buffer.extend_from_slice(part);
            if part.last() != Some(&b'\n') {
                continue;
            }
            let line = std::str::from_utf8(&self.buffer)
                .map_err(|_| response::invalid())?
                .trim_end_matches('\n');
            if let Some(frame) = parse_sse_line(line, &mut self.frame) {
                self.frame_bytes = 0;
                if let Some(chunk) = self.events.accept(frame)? {
                    if self.events.collected.is_none() {
                        let chunk = Bytes::from(format!("data: {chunk}\n\n"));
                        self.output_bytes = self.output_bytes.saturating_add(chunk.len());
                        if self.output_bytes > 2 * 1024 * 1024 {
                            return Err(response::invalid());
                        }
                        self.outputs.push_back(chunk);
                        if self.events.terminal {
                            self.outputs
                                .push_back(Bytes::from_static(b"data: [DONE]\n\n"));
                        }
                    }
                }
            } else if line.is_empty() || line == "\r" {
                self.frame_bytes = 0;
            }
            self.buffer.clear();
        }
        Ok(())
    }

    fn complete(&self) -> Result<(), GatewayError> {
        if self.events.terminal {
            Ok(())
        } else {
            Err(response::invalid())
        }
    }
}

pub async fn accumulate(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
        response,
        "Cohere streaming response",
    )
    .await?;
    decode_sse(&bytes, model)
}

pub(crate) fn decode_sse(
    bytes: &[u8],
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut decoder = Decoder::new(model, true);
    decoder.feed(bytes)?;
    decoder.complete()?;
    unpack(decoder.events.collected.as_ref().unwrap())
}

pub fn translate<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, StreamError<E>>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, StreamError<E>>> + Send + 'static {
    let inner: Pin<Box<dyn Stream<Item = Result<Bytes, StreamError<E>>> + Send>> = Box::pin(inner);
    futures::stream::unfold(
        (inner, Decoder::new(&model, false), Bytes::new(), false),
        |(mut inner, mut decoder, mut pending, ended)| async move {
            if ended {
                return None;
            }
            loop {
                if let Some(chunk) = decoder.outputs.pop_front() {
                    decoder.output_bytes = decoder.output_bytes.saturating_sub(chunk.len());
                    return Some((Ok(chunk), (inner, decoder, pending, false)));
                }
                if decoder.events.terminal {
                    return None;
                }
                // Fixed-size batches bound the queue without depending on HTTP chunk sizes.
                let result = if !pending.is_empty() {
                    let size = pending.len().min(4096);
                    decoder.feed(&pending.split_to(size))
                } else {
                    match inner.next().await {
                        Some(Ok(bytes)) => {
                            pending = bytes;
                            continue;
                        }
                        Some(Err(error)) => {
                            return Some((Err(error), (inner, decoder, pending, true)))
                        }
                        None => decoder.complete(),
                    }
                };
                if let Err(error) = result {
                    let error =
                        StreamError::Protocol(ProtocolStreamError::invalid_data(error.message));
                    return Some((Err(error), (inner, decoder, pending, true)));
                }
            }
        },
    )
}

#[cfg(test)]
mod tests;
