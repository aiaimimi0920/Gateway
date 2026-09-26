//! Realtime WebSocket upgrade, session event dispatch and frame transport.

mod response;
mod session;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod session_contract_tests;

use self::response::run_realtime_response;
use self::session::{normalize_realtime_item, parse_openai_session_tools, RealtimeSession};
use crate::error::GatewayError;
use crate::state::AppState;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::response::Response;
use futures::StreamExt;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn handle_realtime(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let token = bearer_token(&headers);
    Ok(ws
        .on_upgrade(move |socket| async move {
            if let Err(error) =
                run_realtime_socket(socket, state, token, headers, query_params).await
            {
                tracing::warn!(error = %error, "openai realtime socket closed with error");
            }
        })
        .into_response())
}

async fn run_realtime_socket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    token: Option<String>,
    headers: HeaderMap,
    query_params: HashMap<String, String>,
) -> Result<(), GatewayError> {
    let model = query_params
        .get("model")
        .cloned()
        .unwrap_or_else(|| "gpt-5.4".to_string());
    let mut session = RealtimeSession::new(model.clone());

    send_json(
        &mut socket,
        json!({
            "type": "session.created",
            "session": {
                "id": session.session_id,
                "model": model,
            }
        }),
    )
    .await?;

    while let Some(message) = socket.next().await {
        let message = match message {
            Ok(message) => message,
            Err(error) => {
                return Err(GatewayError::server_error(format!(
                    "realtime websocket receive failed: {error}"
                )));
            }
        };

        match message {
            Message::Text(text) => {
                let raw = match serde_json::from_str::<Value>(&text) {
                    Ok(value) => value,
                    Err(error) => {
                        send_error(&mut socket, format!("invalid JSON frame: {error}")).await?;
                        continue;
                    }
                };
                let event_type = raw
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match event_type {
                    "session.update" => {
                        if let Some(next_model) = raw
                            .get("session")
                            .and_then(|value| value.get("model"))
                            .and_then(|value| value.as_str())
                        {
                            session.model = next_model.to_string();
                        }
                        session.instructions = raw
                            .get("session")
                            .and_then(|value| value.get("instructions"))
                            .and_then(|value| value.as_str())
                            .map(str::to_string);
                        if let Some((tools, tool_choice)) =
                            parse_openai_session_tools(raw.get("session").unwrap_or(&Value::Null))
                        {
                            session.tools = tools;
                            session.tool_choice = tool_choice;
                        }
                        send_json(
                            &mut socket,
                            json!({
                                "type": "session.updated",
                                "session": {
                                    "id": session.session_id,
                                    "model": session.model,
                                }
                            }),
                        )
                        .await?;
                    }
                    "conversation.item.create" => {
                        if let Some(item) = raw.get("item") {
                            if let Some(message) = normalize_realtime_item(item) {
                                session.messages.push(message);
                                send_json(
                                    &mut socket,
                                    json!({
                                        "type": "conversation.item.created",
                                        "item": item,
                                    }),
                                )
                                .await?;
                            }
                        }
                    }
                    "response.create" => {
                        run_realtime_response(
                            &mut socket,
                            &state,
                            &token,
                            &headers,
                            &query_params,
                            &mut session,
                            raw.get("response"),
                        )
                        .await?;
                    }
                    "input_audio_buffer.append"
                    | "input_audio_buffer.commit"
                    | "input_audio_buffer.clear" => {
                        send_error(
                            &mut socket,
                            "audio buffers are not supported by the gateway realtime bridge yet",
                        )
                        .await?;
                    }
                    "ping" => {
                        send_json(&mut socket, json!({"type": "pong"})).await?;
                    }
                    _ => {
                        send_error(
                            &mut socket,
                            format!("unsupported realtime event `{event_type}`"),
                        )
                        .await?;
                    }
                }
            }
            Message::Binary(_) => {
                send_error(
                    &mut socket,
                    "binary websocket frames are not supported by the gateway realtime bridge",
                )
                .await?;
            }
            Message::Close(_) => break,
            Message::Ping(payload) => {
                socket.send(Message::Pong(payload)).await.map_err(|error| {
                    GatewayError::server_error(format!(
                        "failed to reply to websocket ping: {error}"
                    ))
                })?;
            }
            Message::Pong(_) => {}
        }
    }

    Ok(())
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string)
}

async fn send_error(
    socket: &mut WebSocket,
    message: impl Into<String>,
) -> Result<(), GatewayError> {
    send_json(
        socket,
        json!({
            "type": "error",
            "error": {
                "message": message.into(),
            }
        }),
    )
    .await
}

async fn send_json(socket: &mut WebSocket, value: Value) -> Result<(), GatewayError> {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .map_err(|error| GatewayError::server_error(format!("websocket send failed: {error}")))
}
