//! Inspect bounded pre-output SSE evidence before handing the response to the caller.
use super::{ByteStream, GatewayError, StreamError};
use crate::protocol::stream_decode::{BoundedSseDecoder, DecodeStep};
use bytes::Bytes;
use futures::StreamExt;

const PREFIX_LIMIT: usize = 64 * 1024;

pub(super) async fn check(mut stream: ByteStream) -> Result<ByteStream, GatewayError> {
    let mut prefix = Vec::new();
    let mut size = 0;
    let mut decoder = BoundedSseDecoder::new(PREFIX_LIMIT);
    while let Some(item) = stream.next().await {
        let bytes = item.map_err(|error| match error {
            StreamError::Protocol(ref protocol) => classify(&protocol.to_string(), ""),
            StreamError::NativeTransport(error) => {
                crate::error::classify_native_network_error(error, "stream_preflight")
            }
            StreamError::Transport(error) => crate::error::classify_network_error(&error, None),
        })?;
        size += bytes.len();
        // Never accumulate an unbounded preamble; hand off without claiming success.
        if size > PREFIX_LIMIT {
            prefix.push(bytes);
            return Ok(replay(prefix, stream));
        }
        decoder.push_chunk(bytes.clone());
        prefix.push(bytes);
        loop {
            match decoder.decode_next() {
                DecodeStep::NeedInput => break,
                DecodeStep::Error(error) => {
                    return Err(GatewayError::server_error(error.to_string()))
                }
                DecodeStep::Frame(frame) => {
                    if frame.data.is_empty() {
                        continue;
                    }
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(&frame.data) else {
                        return Ok(replay(prefix, stream));
                    };
                    let kind = value
                        .get("type")
                        .and_then(|v| v.as_str())
                        .or(frame.event_name.as_deref())
                        .unwrap_or("");
                    let error = value
                        .get("error")
                        .filter(|v| !v.is_null())
                        .or_else(|| value.pointer("/response/error").filter(|v| !v.is_null()));
                    if let Some(error) = error {
                        let message = error
                            .get("message")
                            .and_then(|v| v.as_str())
                            .or_else(|| error.as_str())
                            .unwrap_or("Upstream stream error");
                        let code = error
                            .get("code")
                            .or_else(|| error.get("type"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        return Err(classify(&format!("{code}: {message}"), code));
                    }
                    if matches!(kind, "error" | "response.failed" | "response.cancelled") {
                        return Err(GatewayError::server_error(
                            "Upstream stream failed before output",
                        ));
                    }
                    // Only known non-output lifecycle events may be consumed speculatively.
                    if !is_empty_preamble(kind, &value) {
                        return Ok(replay(prefix, stream));
                    }
                }
            }
        }
    }
    Err(GatewayError::server_error(
        "Upstream stream ended before output",
    ))
}

fn replay(prefix: Vec<Bytes>, stream: ByteStream) -> ByteStream {
    Box::pin(futures::stream::iter(prefix.into_iter().map(Ok)).chain(stream))
}

fn is_empty_preamble(kind: &str, value: &serde_json::Value) -> bool {
    match kind {
        "response.created" | "response.in_progress" | "ping" => true,
        "response.output_item.added" => value.get("item").is_some_and(|item| {
            item.get("type").and_then(|v| v.as_str()) == Some("message")
                && item
                    .get("content")
                    .and_then(|v| v.as_array())
                    .is_some_and(Vec::is_empty)
        }),
        "response.content_part.added" => value.get("part").is_some_and(|part| {
            part.get("type").and_then(|v| v.as_str()) == Some("output_text")
                && part.get("text").and_then(|v| v.as_str()) == Some("")
        }),
        "" => value
            .get("choices")
            .and_then(|v| v.as_array())
            .is_some_and(|choices| {
                !choices.is_empty()
                    && choices.iter().all(|choice| {
                        choice.get("finish_reason").is_none_or(|v| v.is_null())
                            && choice.get("delta").and_then(|v| v.as_object()).is_some_and(
                                |delta| {
                                    delta.iter().all(|(key, value)| match key.as_str() {
                                        "role" => value.as_str() == Some("assistant"),
                                        "content" => value.is_null() || value.as_str() == Some(""),
                                        _ => false,
                                    })
                                },
                            )
                    })
            }),
        _ => false,
    }
}

fn classify(message: &str, code: &str) -> GatewayError {
    // A structured model refusal proves no inference started; arbitrary transport
    // errors must not acquire the non-idempotent admission-rejection allowance.
    let model_missing = code == "model_not_found";
    let body = serde_json::json!({"error": {"message": message,
        "code": if model_missing { "model_not_found" } else { "upstream_stream_error" }}});
    let mut error = crate::error::classify_upstream_error(
        if model_missing { 404 } else { 502 },
        &body.to_string(),
        None,
    );
    if model_missing {
        error.code = Some("upstream_stream_model_not_found".into());
        error.fallback_hint = crate::error::FallbackHint::FallbackProvider {
            reason: "Upstream rejected this model before output; try another candidate".into(),
        };
    }
    error
}
