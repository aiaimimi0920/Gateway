use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use futures::StreamExt;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::http::request_headers::apply_public_request_headers;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::{gemini_api, openai, tool_inject};
use crate::state::AppState;

use super::ws_bridge::{OpenAiSseAccumulator, PendingToolCall};

mod payload;
use payload::{build_gemini_live_turn_body, build_gemini_setup_body, clone_present_value};

pub async fn handle_bidi_generate_content(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let token = bearer_token(&headers);
    Ok(ws
        .on_upgrade(move |socket| async move {
            if let Err(error) =
                run_gemini_live_socket(socket, state, token, headers, query_params).await
            {
                tracing::warn!(error = %error, "gemini live socket closed with error");
            }
        })
        .into_response())
}

struct GeminiLiveSession {
    model: String,
    tools: Vec<crate::protocol::canonical::CanonicalTool>,
    tool_choice: Option<Value>,
    reasoning: Option<Value>,
    system_instruction: Option<Value>,
    messages: Vec<CanonicalMessage>,
}

impl GeminiLiveSession {
    fn new(model: String) -> Self {
        Self {
            model,
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            system_instruction: None,
            messages: Vec::new(),
        }
    }
}

async fn run_gemini_live_socket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    token: Option<String>,
    headers: HeaderMap,
    query_params: HashMap<String, String>,
) -> Result<(), GatewayError> {
    let mut session = GeminiLiveSession::new("gpt-5.4".to_string());

    while let Some(message) = socket.next().await {
        let message = match message {
            Ok(message) => message,
            Err(error) => {
                return Err(GatewayError::server_error(format!(
                    "gemini live websocket receive failed: {error}"
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
                if let Some(setup) = raw.get("setup") {
                    apply_gemini_setup(&mut session, setup)?;
                    send_json(&mut socket, json!({"setupComplete": {}})).await?;
                    continue;
                }
                if let Some(client_content) = raw.get("clientContent") {
                    append_gemini_turns(&mut session, client_content.get("turns"))?;
                    if client_content
                        .get("turnComplete")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(true)
                    {
                        run_gemini_live_turn(
                            &mut socket,
                            &state,
                            &token,
                            &headers,
                            &query_params,
                            &mut session,
                        )
                        .await?;
                    }
                    continue;
                }
                if let Some(tool_response) = raw.get("toolResponse") {
                    append_gemini_tool_responses(&mut session, tool_response)?;
                    run_gemini_live_turn(
                        &mut socket,
                        &state,
                        &token,
                        &headers,
                        &query_params,
                        &mut session,
                    )
                    .await?;
                    continue;
                }
                if raw.get("realtimeInput").is_some() {
                    send_error(
                        &mut socket,
                        "realtimeInput audio/video is not supported by the gateway Gemini Live bridge yet",
                    )
                    .await?;
                    continue;
                }
                send_error(&mut socket, "unsupported Gemini Live client message").await?;
            }
            Message::Binary(_) => {
                send_error(
                    &mut socket,
                    "binary websocket frames are not supported by the Gemini Live bridge",
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

fn apply_gemini_setup(session: &mut GeminiLiveSession, setup: &Value) -> Result<(), GatewayError> {
    let normalized_model = setup
        .get("model")
        .and_then(|value| value.as_str())
        .map(|value| value.trim_start_matches("models/").to_string())
        .unwrap_or_else(|| session.model.clone());
    let body = build_gemini_setup_body(setup, &normalized_model);
    let req = gemini_api::normalize_generate_content(body, None, true)?;
    if let Some(model) = req.requested_model.clone() {
        session.model = model;
    }
    session.tools = req.tools;
    session.tool_choice = req.tool_choice;
    session.reasoning = req.reasoning.filter(|value| !value.is_null());
    session.system_instruction = clone_present_value(setup.get("systemInstruction"));
    Ok(())
}

fn append_gemini_turns(
    session: &mut GeminiLiveSession,
    raw_turns: Option<&Value>,
) -> Result<(), GatewayError> {
    let body = json!({
        "model": session.model,
        "contents": raw_turns.cloned().unwrap_or_else(|| json!([])),
    });
    let req = gemini_api::normalize_generate_content(body, None, false)?;
    session.messages.extend(req.messages);
    Ok(())
}

fn append_gemini_tool_responses(
    session: &mut GeminiLiveSession,
    tool_response: &Value,
) -> Result<(), GatewayError> {
    let responses = tool_response
        .get("functionResponses")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let contents = responses
        .into_iter()
        .map(|response| {
            json!({
                "role": "user",
                "parts": [{
                    "functionResponse": response
                }]
            })
        })
        .collect::<Vec<_>>();
    let body = json!({
        "model": session.model,
        "contents": contents,
    });
    let req = gemini_api::normalize_generate_content(body, None, false)?;
    session.messages.extend(req.messages);
    Ok(())
}

async fn run_gemini_live_turn(
    socket: &mut WebSocket,
    state: &Arc<AppState>,
    token: &Option<String>,
    headers: &HeaderMap,
    query_params: &HashMap<String, String>,
    session: &mut GeminiLiveSession,
) -> Result<(), GatewayError> {
    let request_body = build_gemini_live_turn_body(session);
    let mut messages = Vec::new();
    if let Some(system_text) = extract_instruction_text(session.system_instruction.as_ref()) {
        messages.push(CanonicalMessage {
            role: MessageRole::System,
            content: vec![ContentPart::Text { text: system_text }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        });
    }
    messages.extend(session.messages.clone());

    let request = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::GeminiLive,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(session.model.clone()),
        stream: true,
        messages,
        tools: session.tools.clone(),
        tool_choice: session.tool_choice.clone(),
        reasoning: session.reasoning.clone(),
        metadata: None,
        raw_body: request_body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };
    let request_messages_text = request.messages_text();
    let request_tools = request.tools.clone();
    let request_tool_choice = request.tool_choice.clone();
    let mut ctx = PipelineContext::new(request, token.clone());
    apply_public_request_headers(
        &mut ctx,
        headers,
        &state.config,
        state.console_auth.as_ref(),
        &["x-goog-api-key"],
    )?;
    ctx.query_params = query_params.clone();

    let mut assistant_text = String::new();
    let mut pending_tools: HashMap<usize, PendingToolCall> = HashMap::new();
    let mut finish_reason = None;
    let mut accumulator = OpenAiSseAccumulator::default();

    match run_pipeline(ctx, state).await? {
        PipelineOutput::Sse(mut stream) => {
            while let Some(chunk) = stream.next().await {
                let bytes = chunk.map_err(|error| {
                    GatewayError::server_error(format!(
                        "gemini live bridge upstream stream failed: {error}"
                    ))
                })?;
                for frame in accumulator.push_bytes(bytes.as_ref()) {
                    if frame.done {
                        continue;
                    }
                    if let Some(delta) = frame.content_delta.as_deref() {
                        assistant_text.push_str(delta);
                        send_json(
                            socket,
                            json!({
                                "serverContent": {
                                    "modelTurn": {
                                        "role": "model",
                                        "parts": [{"text": delta}],
                                    }
                                }
                            }),
                        )
                        .await?;
                    }
                    for delta in &frame.tool_call_deltas {
                        let entry = pending_tools.entry(delta.index).or_insert_with(|| {
                            PendingToolCall::new(format!("tool_{}", uuid::Uuid::new_v4()))
                        });
                        entry.apply_delta(delta);
                    }
                    if frame.finish_reason.is_some() {
                        finish_reason = frame.finish_reason;
                    }
                }
            }
        }
        PipelineOutput::Json(value) => {
            let canonical_resp = openai::unpack_openai_response(&value)?;
            assistant_text = canonical_resp.text;
            finish_reason = canonical_resp.finish_reason;
            for (index, tool_call) in canonical_resp.tool_calls.iter().enumerate() {
                pending_tools.insert(
                    index,
                    PendingToolCall {
                        id: tool_call
                            .id
                            .clone()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                        item_id: format!("tool_{}", uuid::Uuid::new_v4()),
                        name: tool_call.name.clone().unwrap_or_default(),
                        arguments: tool_call.arguments.clone().unwrap_or_default(),
                    },
                );
            }
        }
        PipelineOutput::Binary(_) => {
            return Err(GatewayError::server_error(
                "unexpected binary response for Gemini Live bridge",
            ));
        }
    }

    let has_resolved_tool_calls = pending_tools.values().any(|tool_call| {
        !tool_call.name.trim().is_empty() && !tool_call.arguments.trim().is_empty()
    });
    if !has_resolved_tool_calls && !assistant_text.is_empty() {
        let parse_result = tool_inject::parse_tool_calls_from_text_with_context(
            &assistant_text,
            &request_tools,
            request_tool_choice.as_ref(),
            Some(&request_messages_text),
        );
        if parse_result.had_tool_calls {
            assistant_text = parse_result.clean_text;
            finish_reason = Some("tool_calls".to_string());
            pending_tools.clear();
            for (index, tool_call) in parse_result.tool_calls.iter().enumerate() {
                pending_tools.insert(
                    index,
                    PendingToolCall {
                        id: tool_call
                            .id
                            .clone()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                        item_id: format!("tool_{}", uuid::Uuid::new_v4()),
                        name: tool_call.name.clone().unwrap_or_default(),
                        arguments: tool_call
                            .arguments
                            .clone()
                            .unwrap_or_else(|| "{}".to_string()),
                    },
                );
            }
        }
    }

    if !pending_tools.is_empty() {
        send_json(
            socket,
            json!({
                "toolCall": {
                    "functionCalls": pending_tools
                        .values()
                        .map(|tool_call| json!({
                            "id": tool_call.id,
                            "name": tool_call.name,
                            "args": tool_call.arguments_json(),
                        }))
                        .collect::<Vec<_>>()
                }
            }),
        )
        .await?;
    }

    send_json(
        socket,
        json!({
            "serverContent": {
                "turnComplete": true,
                "interrupted": matches!(finish_reason.as_deref(), Some("length")),
            }
        }),
    )
    .await?;

    let tool_calls = pending_tools
        .into_values()
        .map(|tool_call| tool_call.as_tool_call())
        .collect::<Vec<_>>();
    if !assistant_text.is_empty() || !tool_calls.is_empty() {
        session.messages.push(CanonicalMessage {
            role: MessageRole::Assistant,
            content: if assistant_text.is_empty() {
                Vec::new()
            } else {
                vec![ContentPart::Text {
                    text: assistant_text,
                }]
            },
            name: None,
            tool_call_id: None,
            tool_calls,
        });
    }

    Ok(())
}

fn extract_instruction_text(system_instruction: Option<&Value>) -> Option<String> {
    let system_instruction = system_instruction?;
    let parts = system_instruction.get("parts")?.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
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
            "serverContent": {
                "turnComplete": true,
            },
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

#[cfg(test)]
mod tests;
