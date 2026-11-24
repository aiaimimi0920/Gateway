//! Runtime request planner, buffered executor, and primed real-time SSE streamer for `quick-oauth`.

use super::{
    messages::{extract_client_tool_calls, prepare_turn},
    metadata,
    websocket::{
        connect_and_begin_turn, delete_and_close, ActiveTurnSocket, TurnAccumulator, PING_INTERVAL,
        POST_MESSAGE_GRACE,
    },
    PORTAL_BASE,
};
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind, TokenUsage},
        openai,
        sse_parse::format_sse_event,
        stream_error::{ProtocolStreamError, StreamError},
    },
    routing::candidate::ProviderAccountPayload,
    upstream::{
        canonical_sse::canonical_response_to_openai_sse_bytes, client::UpstreamClient,
        common::RequestPlan, response_types::UpstreamStreamingResponse,
    },
};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::tungstenite::Message;

pub fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    let meta = metadata::read(payload)?;
    let turn = prepare_turn(req, model)?;
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{PORTAL_BASE}/streaming/quickwork-chat-bidirectional-stream"),
        query: vec![("ws_host".to_owned(), meta.ws_host)],
        body: Some(json!({
            "model": turn.model,
            "stream": stream,
        })),
        response_kind: EndpointKind::ChatCompletions,
    })
}

pub async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    let meta = metadata::read(payload)?;
    let turn = prepare_turn(req, model)?;
    let budget = client.timeout.max(Duration::from_secs(10));
    let deadline = Instant::now() + budget;
    let mut active = connect_and_begin_turn(&meta, &payload.api_key, &turn, budget).await?;
    let mut acc = TurnAccumulator::default();
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.tick().await;
    let mut grace_deadline: Option<Instant> = None;

    let outcome: Result<(), GatewayError> = async {
        while !acc.finished {
            let now = Instant::now();
            if now >= deadline {
                return Err(GatewayError::service_unavailable(
                    "Amazon Quick turn timed out waiting for completion",
                )
                .with_code("quick_oauth_turn_timeout"));
            }
            let wait_until = match grace_deadline {
                Some(gd) => gd.min(deadline),
                None => deadline,
            };
            tokio::select! {
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(wait_until)) => {
                    if grace_deadline.is_some_and(|gd| Instant::now() >= gd) {
                        acc.flush_pending_tools();
                        acc.finished = true;
                        break;
                    }
                    return Err(GatewayError::service_unavailable(
                        "Amazon Quick turn timed out waiting for completion",
                    )
                    .with_code("quick_oauth_turn_timeout"));
                }
                _ = ping.tick() => {
                    let _ = active.socket.send(Message::Ping(Vec::new().into())).await;
                }
                next = active.socket.next() => {
                    let Some(frame) = next else {
                        acc.flush_pending_tools();
                        break;
                    };
                    let msg = frame.map_err(|_| {
                        bad_gateway("Amazon Quick WebSocket closed unexpectedly", "quick_oauth_ws_read_failed")
                    })?;
                    acc.apply_ws_message(msg)?;
                    if acc.post_message_grace && grace_deadline.is_none() {
                        grace_deadline = Some(Instant::now() + POST_MESSAGE_GRACE);
                    }
                }
            }
        }
        Ok(())
    }
    .await;

    delete_and_close(active).await;
    outcome?;

    let (clean_text, mut tool_calls) = if turn.has_client_tools {
        extract_client_tool_calls(&acc.text)
    } else {
        (acc.text, Vec::new())
    };
    tool_calls.extend(acc.native_tools);
    if clean_text.is_empty() && tool_calls.is_empty() && acc.usage.is_none() {
        return Err(bad_gateway(
            "Amazon Quick returned an empty conversation turn",
            "quick_oauth_empty_turn",
        ));
    }
    let finish_reason = Some(if tool_calls.is_empty() {
        "stop".to_owned()
    } else {
        "tool_calls".to_owned()
    });
    Ok(CanonicalRelayResponse {
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: None,
        model: turn.model,
        text: clean_text,
        sources: Vec::new(),
        usage: acc.usage,
        tool_calls,
        upstream_status: Some(200),
        finish_reason,
    })
}

pub async fn execute_stream(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    let meta = metadata::read(payload)?;
    let turn = prepare_turn(req, model)?;
    if turn.has_client_tools {
        let canonical = execute(client, payload, req, model).await?;
        let frames = canonical_response_to_openai_sse_bytes(req, model, &canonical);
        return Ok(UpstreamStreamingResponse::Bytes(Box::pin(
            futures::stream::iter(frames.into_iter().map(Ok)),
        )));
    }

    let budget = client.timeout.max(Duration::from_secs(10));
    let deadline = Instant::now() + budget;
    let mut active = connect_and_begin_turn(&meta, &payload.api_key, &turn, budget).await?;
    let mut acc = TurnAccumulator::default();
    let response_id = format!("chatcmpl_quick_{}", uuid::Uuid::new_v4().simple());
    let created_at = unix_secs();
    let out_model = req
        .requested_model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&turn.model)
        .to_owned();

    let first_deltas = match prime_first_deltas(&mut active, &mut acc, deadline).await {
        Ok(deltas) => deltas,
        Err(err) => {
            delete_and_close(active).await;
            return Err(err);
        }
    };

    let (tx, rx) = mpsc::channel::<Result<Bytes, StreamError<rquest::Error>>>(32);
    tokio::spawn(drive_sse_stream(
        active,
        acc,
        first_deltas,
        response_id,
        created_at,
        out_model,
        deadline,
        tx,
    ));
    Ok(UpstreamStreamingResponse::NativeEvents(Box::pin(
        ReceiverStream::new(rx),
    )))
}

async fn prime_first_deltas(
    active: &mut ActiveTurnSocket,
    acc: &mut TurnAccumulator,
    deadline: Instant,
) -> Result<Vec<String>, GatewayError> {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.tick().await;
    while !acc.finished {
        if Instant::now() >= deadline {
            return Err(GatewayError::service_unavailable(
                "Amazon Quick stream timed out before first token",
            )
            .with_code("quick_oauth_stream_timeout"));
        }
        tokio::select! {
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                return Err(
                    GatewayError::service_unavailable("Amazon Quick stream timed out before first token")
                        .with_code("quick_oauth_stream_timeout"),
                );
            }
            _ = ping.tick() => {
                let _ = active.socket.send(Message::Ping(Vec::new().into())).await;
            }
            next = active.socket.next() => {
                let Some(frame) = next else {
                    return Err(bad_gateway(
                        "Amazon Quick WebSocket closed before emitting events",
                        "quick_oauth_closed_early",
                    ));
                };
                let msg = frame.map_err(|_| {
                    bad_gateway("Amazon Quick WebSocket read failed", "quick_oauth_ws_read_failed")
                })?;
                let step = acc.apply_ws_message(msg)?;
                if !step.text_deltas.is_empty() || step.finished {
                    return Ok(step.text_deltas);
                }
            }
        }
    }
    Ok(Vec::new())
}

#[allow(clippy::too_many_arguments)]
async fn drive_sse_stream(
    mut active: ActiveTurnSocket,
    mut acc: TurnAccumulator,
    initial_deltas: Vec<String>,
    response_id: String,
    created_at: i64,
    model: String,
    deadline: Instant,
    tx: mpsc::Sender<Result<Bytes, StreamError<rquest::Error>>>,
) {
    for piece in initial_deltas {
        if send_delta(&tx, &response_id, created_at, &model, &piece)
            .await
            .is_err()
        {
            delete_and_close(active).await;
            return;
        }
    }
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.tick().await;
    let mut grace_deadline = acc
        .post_message_grace
        .then(|| Instant::now() + POST_MESSAGE_GRACE);

    while !acc.finished {
        let wait_until = grace_deadline.map_or(deadline, |gd| gd.min(deadline));
        tokio::select! {
            _ = tx.closed() => {
                delete_and_close(active).await;
                return;
            }
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(wait_until)) => {
                if grace_deadline.is_some_and(|gd| Instant::now() >= gd) {
                    acc.flush_pending_tools();
                    break;
                }
                let err = GatewayError::service_unavailable("Amazon Quick stream timed out")
                    .with_code("quick_oauth_stream_timeout");
                let _ = tx.send(Err(StreamError::Protocol(ProtocolStreamError::classified("quick_oauth_stream_timeout", err)))).await;
                delete_and_close(active).await;
                return;
            }
            _ = ping.tick() => {
                let _ = active.socket.send(Message::Ping(Vec::new().into())).await;
            }
            next = active.socket.next() => {
                let Some(frame) = next else {
                    acc.flush_pending_tools();
                    break;
                };
                let step = match frame.map_err(|_| {
                    bad_gateway("Amazon Quick WebSocket stream interrupted", "quick_oauth_ws_read_failed")
                }).and_then(|msg| acc.apply_ws_message(msg)) {
                    Ok(step) => step,
                    Err(err) => {
                        let _ = tx.send(Err(StreamError::Protocol(ProtocolStreamError::classified("quick_oauth_stream_error", err)))).await;
                        delete_and_close(active).await;
                        return;
                    }
                };
                if acc.post_message_grace && grace_deadline.is_none() {
                    grace_deadline = Some(Instant::now() + POST_MESSAGE_GRACE);
                }
                for piece in step.text_deltas {
                    if send_delta(&tx, &response_id, created_at, &model, &piece).await.is_err() {
                        delete_and_close(active).await;
                        return;
                    }
                }
            }
        }
    }

    let stop_chunk = build_stop_chunk(&response_id, created_at, &model, acc.usage.as_ref());
    let _ = tx
        .send(Ok(Bytes::from(format_sse_event(
            None,
            &stop_chunk.to_string(),
        ))))
        .await;
    let _ = tx
        .send(Ok(Bytes::from(format_sse_event(None, "[DONE]"))))
        .await;
    delete_and_close(active).await;
}

fn bad_gateway(message: &str, code: &'static str) -> GatewayError {
    let mut err = GatewayError::service_unavailable(message).with_code(code);
    err.http_status = Some(502);
    err
}

async fn send_delta(
    tx: &mpsc::Sender<Result<Bytes, StreamError<rquest::Error>>>,
    response_id: &str,
    created_at: i64,
    model: &str,
    piece: &str,
) -> Result<(), ()> {
    let chunk = openai::build_chat_completions_delta(response_id, created_at, model, piece);
    tx.send(Ok(Bytes::from(format_sse_event(None, &chunk.to_string()))))
        .await
        .map_err(|_| ())
}

fn build_stop_chunk(
    response_id: &str,
    created_at: i64,
    model: &str,
    usage: Option<&TokenUsage>,
) -> serde_json::Value {
    let mut stop = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop",
        }],
        "usage": usage.map(|u| json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        })).unwrap_or_else(|| json!(null)),
    });
    if let Some(u) = usage {
        if let Some(cached) = u.cache_read_input_tokens {
            stop["usage"]["prompt_tokens_details"] = json!({ "cached_tokens": cached });
        }
        if let Some(created) = u.cache_creation_input_tokens {
            stop["usage"]["cache_creation_input_tokens"] = json!(created);
        }
    }
    stop
}

fn unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
