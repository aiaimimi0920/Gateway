//! Inspect one bounded frame, then preserve native SSE bytes (including id/retry).
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::ProtocolStreamError;
use bytes::Bytes;
use futures::{Stream, StreamExt, TryStreamExt};
use serde_json::Value;
use std::pin::Pin;
type ByteStream<E> = Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>;

pub fn native_or_translate<E>(
    source: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    multimodal: bool,
    message_format: bool,
    incremental: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    futures::stream::once(async move {
        let mut source = Box::pin(source);
        let mut prefix = Vec::new();
        let mut size = 0usize;
        let mut decoder = BoundedSseDecoder::new(MAX_TRANSLATED_SSE_FRAME_BYTES);
        loop {
            match decoder.decode_next() {
                DecodeStep::Frame(frame) => {
                    let value: Value = serde_json::from_str(&frame.data)
                        .map_err(|e| E::from(ProtocolStreamError::from(e)))?;
                    let native = value.get("output").is_some() || value.get("code").is_some();
                    let replay = futures::stream::iter(prefix.into_iter().map(Ok)).chain(source);
                    let stream: ByteStream<E> = if native {
                        Box::pin(replay)
                    } else {
                        Box::pin(super::stream::translate(
                            replay,
                            model,
                            true,
                            multimodal,
                            message_format,
                            incremental,
                        ))
                    };
                    return Ok::<_, E>(stream);
                }
                DecodeStep::Error(error) => return Err(error.into()),
                DecodeStep::NeedInput => match source.next().await {
                    Some(Ok(chunk)) => {
                        size = size.saturating_add(chunk.len());
                        if size > MAX_TRANSLATED_SSE_FRAME_BYTES {
                            return Err(ProtocolStreamError::invalid_data(
                                "DashScope first-event buffer exceeded limit",
                            )
                            .into());
                        }
                        prefix.push(chunk.clone());
                        decoder.push_chunk(chunk);
                    }
                    Some(Err(error)) => return Err(error),
                    None => {
                        return Err(ProtocolStreamError::invalid_data(
                            "DashScope stream ended before its first event",
                        )
                        .into())
                    }
                },
            }
        }
    })
    .try_flatten()
}
