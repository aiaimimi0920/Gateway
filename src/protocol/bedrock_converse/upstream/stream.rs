//! Lazy frame-to-chat projection: drain one event, retain backpressure, drop on terminal error.
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};

use super::super::public_tools::PublicTools;
use super::{capture::Capture, frame::Decoder, relay::EVENT_FIELD};
use crate::protocol::stream_error::ProtocolStreamError;

pub fn translate<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    translate_scoped(inner, model, None)
}

pub(crate) fn translate_scoped<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    scope: Option<crate::protocol::bedrock_turn::continuation::Continuation>,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    translate_projected(inner, model, scope, None)
}

pub(crate) fn translate_public<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    tools: PublicTools,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    translate_projected(inner, model, None, Some(tools))
}

fn translate_projected<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    scope: Option<crate::protocol::bedrock_turn::continuation::Continuation>,
    tools: Option<PublicTools>,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    let inner = Box::pin(inner);
    let public = tools.is_some();
    let decoder = if public {
        Decoder::strict_json()
    } else {
        Decoder::default()
    };
    let state = tools.map(Capture::public).unwrap_or_default();
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    futures::stream::unfold(
        Some((inner, decoder, state, model, id, created, scope)),
        move |owned| async move {
            let (mut inner, mut decoder, mut state, model, id, created, scope) = owned?;
            loop {
                match decoder.next() {
                    Ok(Some(event)) => match state.observe(&event) {
                        Ok(delta) => {
                            if public && delta.as_object().is_some_and(serde_json::Map::is_empty) {
                                continue;
                            }
                            let mut body = chunk(&id, &model, created, delta, None, None);
                            if !public {
                                body[EVENT_FIELD] = super::relay::envelope(state.count - 1, event);
                            }
                            let chunk = match encode(body) {
                                Ok(chunk) => chunk,
                                Err(error) => return Some((Err(error.into()), None)),
                            };
                            return Some((
                                Ok(chunk),
                                Some((inner, decoder, state, model, id, created, scope)),
                            ));
                        }
                        Err(error) => return Some((Err(error.into()), None)),
                    },
                    Err(error) => return Some((Err(error.into()), None)),
                    Ok(None) => {}
                }
                match inner.next().await {
                    Some(Ok(bytes)) => decoder.push(bytes),
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => {
                        let completed = decoder.finish().and_then(|()| state.finish());
                        return Some((
                            completed
                                .and_then(|mut completion| {
                                    if let Some(turn) = &mut completion.turn {
                                        turn.continuation = scope;
                                        crate::protocol::bedrock_turn::wire::validate(turn)
                                            .map_err(|_| {
                                                super::invalid("invalid continuation scope")
                                            })?;
                                    }
                                    let mut body = chunk(
                                        &id,
                                        &model,
                                        created,
                                        json!({}),
                                        Some(completion.reason),
                                        Some(if public {
                                            // Public Chat validates these counters; native cache aliases
                                            // (including null/unknown) are not OpenAI usage fields.
                                            crate::protocol::openai::chat_usage(&completion.usage)
                                        } else {
                                            completion.usage_json()
                                        }),
                                    );
                                    if !public {
                                        completion.project(&mut body);
                                    }
                                    let mut bytes = encode(body)?.to_vec();
                                    bytes.extend_from_slice(b"data: [DONE]\n\n");
                                    Ok(Bytes::from(bytes))
                                })
                                .map_err(Into::into),
                            None,
                        ));
                    }
                }
            }
        },
    )
}

fn chunk(
    id: &str,
    model: &str,
    created: u64,
    delta: Value,
    reason: Option<&str>,
    usage: Option<Value>,
) -> Value {
    let mut body = json!({"id":id,"model":model,"created":created,"object":"chat.completion.chunk",
        "choices":[{"index":0,"delta":delta,"finish_reason":reason}]});
    if let Some(usage) = usage {
        body["usage"] = usage;
    }
    body
}

fn encode(body: Value) -> Result<Bytes, ProtocolStreamError> {
    let bytes = format!("data: {body}\n\n");
    if bytes.len() > crate::protocol::stream_decode::MAX_TRANSLATED_SSE_FRAME_BYTES {
        return Err(super::invalid("normalized event exceeds frame byte limit"));
    }
    Ok(Bytes::from(bytes))
}
