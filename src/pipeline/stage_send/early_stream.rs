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
            StreamError::Protocol(ref protocol) => {
                classify(&protocol.to_string(), "upstream_stream_error")
            }
            StreamError::NativeTransport(error) => {
                crate::error::classify_native_network_error(error, "stream_preflight")
            }
            StreamError::Transport(error) => crate::error::classify_network_error(&error, None),
        })?;
        // Empty chunks carry no SSE bytes; do not retain an unbounded vector of them.
        if bytes.is_empty() {
            continue;
        }
        size = usize::saturating_add(size, bytes.len());
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
                DecodeStep::Frame(frame) => match inspect_frame(frame) {
                    FrameDecision::Continue => continue,
                    FrameDecision::Replay => return Ok(replay(prefix, stream)),
                    FrameDecision::Reject(error) => return Err(error),
                },
            }
        }
    }
    if let Some(frame) = decoder.flush_pending_frame() {
        match inspect_frame(frame) {
            FrameDecision::Continue => {}
            FrameDecision::Replay => return Ok(replay(prefix, stream)),
            FrameDecision::Reject(error) => return Err(error),
        }
    }
    Err(GatewayError::server_error(
        "Upstream stream ended before output",
    ))
}

enum FrameDecision {
    Continue,
    Replay,
    Reject(GatewayError),
}

fn inspect_frame(frame: crate::protocol::sse_parse::SseFrame) -> FrameDecision {
    if frame.data.is_empty() {
        return FrameDecision::Continue;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&frame.data) else {
        return FrameDecision::Replay;
    };
    let kind = value
        .get("type")
        .and_then(|v| v.as_str())
        .or(frame.event_name.as_deref())
        .unwrap_or("");
    // A response handle is observable work even before text. A later failure
    // must not convert that creation into permission for a fresh generation.
    if value
        .pointer("/response/id")
        .and_then(|id| id.as_str())
        .is_some_and(|id| !id.trim().is_empty())
    {
        return FrameDecision::Replay;
    }
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
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .or_else(|| {
                error.get("type").and_then(|v| v.as_str()).filter(|v| {
                    !v.is_empty()
                        && !matches!(
                            *v,
                            "new_api_error"
                                | "one_api_error"
                                | "openai_error"
                                | "rix_api_error"
                                | "shell_api_error"
                        )
                })
            })
            .unwrap_or("");
        // Keep known admission messages intact for their narrow classifiers.
        if matches!(code, "" | "channel_not_found" | "model_not_available")
            || crate::error::is_account_balance_rejection(403, message)
            || crate::error::is_model_pricing_failure(502, Some(code), Some(message))
        {
            return FrameDecision::Reject(classify(message, code));
        }
        return FrameDecision::Reject(classify(&format!("{code}: {message}"), code));
    }
    if matches!(kind, "error" | "response.failed" | "response.cancelled") {
        return FrameDecision::Reject(GatewayError::server_error(
            "Upstream stream failed before output",
        ));
    }
    // Only known non-output lifecycle events may be consumed speculatively.
    if is_empty_preamble(kind, &value) {
        FrameDecision::Continue
    } else {
        FrameDecision::Replay
    }
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
    let balance_rejected = crate::error::is_account_balance_rejection(403, message);
    let pricing_rejected = crate::error::is_model_pricing_failure(502, Some(code), Some(message));
    let preserved = model_missing
        || pricing_rejected
        || matches!(
            code,
            "get_channel_failed" | "channel_not_found" | "model_not_available"
        );
    let wire_code = if preserved {
        Some(code)
    } else if code.is_empty() {
        None
    } else {
        Some("upstream_stream_error")
    };
    let body = serde_json::json!({"error": {"message": message, "code": wire_code}});
    let mut error = crate::error::classify_upstream_error(
        // SSE has HTTP 200; synthesize the semantic admission status only before handoff.
        if model_missing {
            404
        } else if balance_rejected {
            402
        } else {
            502
        },
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
