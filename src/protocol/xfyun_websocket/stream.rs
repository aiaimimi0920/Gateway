use super::connection::connect_and_send;
use super::frames::{build_openai_sse_frames, parse_ws_message};
use super::{OpenAiSseByteStream, XfyunWsSocket};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::sse_parse::format_sse_event;
use crate::routing::candidate::ProviderAccountPayload;
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use std::collections::VecDeque;

struct XfyunWsStreamState {
    socket: XfyunWsSocket,
    pending: VecDeque<Bytes>,
    model: String,
    response_id: String,
    created_at: i64,
    finished: bool,
}

pub async fn execute_stream_as_openai_sse(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<OpenAiSseByteStream, GatewayError> {
    let socket = connect_and_send(payload, req, model).await?;
    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created_at = unix_timestamp_secs();
    let mut state = XfyunWsStreamState {
        socket,
        pending: VecDeque::new(),
        model: model.to_string(),
        response_id,
        created_at,
        finished: false,
    };
    prime_stream_state(&mut state).await?;

    let stream = futures::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(bytes) = state.pending.pop_front() {
                return Some((Ok(bytes), state));
            }
            if state.finished {
                return None;
            }
            match state.socket.next().await {
                Some(message) => match message {
                    Ok(message) => match parse_ws_message(Ok(message), &state.model) {
                        Ok(parsed) => {
                            state.pending.extend(build_openai_sse_frames(
                                &parsed,
                                &state.model,
                                &state.response_id,
                                state.created_at,
                            ));
                            if parsed.done {
                                state.finished = true;
                            }
                        }
                        Err(error) => {
                            state.pending.push_back(Bytes::from(format_sse_event(
                                Some("error"),
                                &json!({
                                    "message": error.message,
                                    "code": error.code,
                                })
                                .to_string(),
                            )));
                            state
                                .pending
                                .push_back(Bytes::from_static(b"data: [DONE]\n\n"));
                            state.finished = true;
                        }
                    },
                    Err(_) => {
                        state.finished = true;
                    }
                },
                None => {
                    state.finished = true;
                }
            }
        }
    });

    Ok(Box::pin(stream))
}

async fn prime_stream_state(state: &mut XfyunWsStreamState) -> Result<(), GatewayError> {
    while state.pending.is_empty() && !state.finished {
        let Some(message) = state.socket.next().await else {
            return Err(GatewayError::service_unavailable(
                "XFYun native WebSocket upstream closed before the first response frame",
            )
            .with_code("xfyun_websocket_closed_early")
            .with_provider("xfyun_websocket_compatible"));
        };
        let parsed = parse_ws_message(message, &state.model)?;
        state.pending.extend(build_openai_sse_frames(
            &parsed,
            &state.model,
            &state.response_id,
            state.created_at,
        ));
        if parsed.done {
            state.finished = true;
        }
    }
    Ok(())
}

fn unix_timestamp_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
