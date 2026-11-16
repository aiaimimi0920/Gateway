//! Dedicated Accio completion/error boundary, with bounded incremental decoding and no spawned task.
use super::{
    native_decode::Decoder,
    native_response::Collector,
    stream_translate::{translate_events_with_state, TranslatorState},
};
use crate::{
    error::GatewayError,
    protocol::{
        canonical::CanonicalRelayResponse,
        stream_error::{ProtocolStreamError, StreamError},
        upstream_body::collect_bounded_upstream_body,
    },
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::pin::Pin;

pub async fn accumulate_accio_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = collect_bounded_upstream_body(response, "Accio streaming adapter body").await?;
    if let Some(error) = super::detect_accio_html_challenge(&body) {
        return Err(error);
    }
    let mut decoder = Decoder::new();
    let mut collector = Collector::new(model);
    for chunk in body.chunks(4096) {
        collector.accept(decoder.feed(chunk)?);
    }
    collector.accept(decoder.complete()?);
    Ok(collector.finish())
}

pub fn translate_accio_stream_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, StreamError<E>>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, StreamError<E>>> + Send + 'static {
    let inner: Pin<Box<dyn Stream<Item = Result<Bytes, StreamError<E>>> + Send>> = Box::pin(inner);
    futures::stream::unfold(
        (
            inner,
            Decoder::new(),
            TranslatorState::new(model),
            Bytes::new(),
            false,
        ),
        |(mut inner, mut decoder, mut render, mut pending, mut ended)| async move {
            loop {
                if let Some(output) = render.outputs.pop_front() {
                    return Some((
                        Ok(Bytes::from(output)),
                        (inner, decoder, render, pending, ended),
                    ));
                }
                if ended {
                    return None;
                }
                let events = if !pending.is_empty() {
                    let size = pending.len().min(4096);
                    decoder.feed(&pending.split_to(size))
                } else {
                    match inner.next().await {
                        Some(Ok(bytes)) => {
                            pending = bytes;
                            continue;
                        }
                        Some(Err(error)) => {
                            return Some((Err(error), (inner, decoder, render, pending, true)))
                        }
                        None => {
                            ended = true;
                            decoder.complete()
                        }
                    }
                };
                match events {
                    Ok(events) => {
                        let outputs = translate_events_with_state(events, &mut render);
                        render.outputs.extend(outputs);
                    }
                    Err(error) => {
                        let prefix = if error.code.as_deref() == Some("accio_invalid_stream") {
                            "accio_stream_invalid"
                        } else {
                            "accio_stream_upstream_error"
                        };
                        let error = StreamError::Protocol(ProtocolStreamError::invalid_data(
                            format!("{prefix}: {}", error.message),
                        ));
                        return Some((Err(error), (inner, decoder, render, pending, true)));
                    }
                }
            }
        },
    )
}
