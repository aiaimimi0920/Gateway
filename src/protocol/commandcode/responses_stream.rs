//! Frame before publishing: upstream failure bodies and premature terminal events never escape.
use crate::protocol::{
    stream_decode::{BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};

pub(crate) fn guard<S, E>(stream: S, model: String) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    let state = (
        Some(Box::pin(stream)),
        BoundedSseDecoder::new_strict(MAX_TRANSLATED_SSE_FRAME_BYTES),
        None::<Bytes>,
        model,
    );
    futures::stream::unfold(
        state,
        |(mut stream, mut decoder, mut terminal, model)| async move {
            stream.as_ref()?;
            let mut steps = 0usize;
            loop {
                steps += 1;
                if steps % 32 == 0 {
                    tokio::task::yield_now().await;
                }
                match decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        if frame.data == "[DONE]" {
                            if terminal.is_some() && frame.event_name.is_none() {
                                continue;
                            }
                        } else if terminal.is_none() {
                            if let Ok((bytes, is_terminal)) = prepare_frame(frame, &model) {
                                if is_terminal {
                                    terminal = Some(bytes);
                                    continue;
                                }
                                return Some((Ok(bytes), (stream, decoder, terminal, model)));
                            }
                        }
                        return Some((
                            Err(super::stream_error().into()),
                            (None, decoder, None, model),
                        ));
                    }
                    DecodeStep::Error(_) => {
                        return Some((
                            Err(super::stream_error().into()),
                            (None, decoder, None, model),
                        ))
                    }
                    DecodeStep::NeedInput => {}
                }
                match stream.as_mut().unwrap().next().await {
                    Some(Ok(bytes)) => decoder.push_chunk(bytes),
                    Some(Err(_)) => {
                        return Some((
                            Err(super::stream_error().into()),
                            (None, decoder, None, model),
                        ))
                    }
                    None => {
                        let result = if decoder.finish_strict().is_ok() {
                            terminal.take().ok_or_else(|| super::stream_error().into())
                        } else {
                            Err(super::stream_error().into())
                        };
                        return Some((result, (None, decoder, None, model)));
                    }
                }
            }
        },
    )
}

#[cfg(test)]
#[path = "responses_stream_tests.rs"]
mod tests;

fn prepare_frame(
    frame: crate::protocol::sse_parse::SseFrame,
    model: &str,
) -> Result<(Bytes, bool), ()> {
    let mut value = crate::protocol::unique_json::parse(frame.data.as_bytes()).map_err(|_| ())?;
    let kind = value["type"].as_str().ok_or(())?.to_owned();
    // The native type becomes an SSE field; opaque extensions cannot inject event lines.
    if !kind.starts_with("response.")
        || kind.contains(['\r', '\n'])
        || frame
            .event_name
            .as_deref()
            .is_some_and(|event| event != kind)
        || matches!(kind.as_str(), "response.cancelled" | "response.failed")
        || !value["error"].is_null()
        || !value["response"]["error"].is_null()
    {
        return Err(());
    }
    let terminal = matches!(kind.as_str(), "response.completed" | "response.incomplete");
    if terminal {
        super::responses::validate_native(&value["response"]).map_err(|_| ())?;
        if value["response"]["status"].as_str() != kind.strip_prefix("response.") {
            return Err(());
        }
    }
    // Upstream identity was observed before this guard. Do not rewrite model-shaped tool content.
    if matches!(
        kind.as_str(),
        "response.created"
            | "response.in_progress"
            | "response.queued"
            | "response.completed"
            | "response.incomplete"
    ) && value["response"]["model"].is_string()
    {
        value["response"]["model"] = model.into();
    }
    Ok((
        Bytes::from(format!("event: {kind}\ndata: {value}\n\n")),
        terminal,
    ))
}
