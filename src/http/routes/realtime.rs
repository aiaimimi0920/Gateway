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
use crate::protocol::{openai, responses, tool_inject};
use crate::state::AppState;

use super::ws_bridge::{OpenAiSseAccumulator, PendingToolCall};

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

struct RealtimeSession {
    session_id: String,
    model: String,
    instructions: Option<String>,
    tools: Vec<crate::protocol::canonical::CanonicalTool>,
    tool_choice: Option<Value>,
    messages: Vec<CanonicalMessage>,
}

impl RealtimeSession {
    fn new(model: String) -> Self {
        Self {
            session_id: format!("sess_{}", uuid::Uuid::new_v4()),
            model,
            instructions: None,
            tools: Vec::new(),
            tool_choice: None,
            messages: Vec::new(),
        }
    }

    fn build_request(&self, override_response: Option<&Value>) -> CanonicalRelayRequest {
        let mut messages = Vec::new();
        let response = override_response.unwrap_or(&Value::Null);
        let response_instructions = response
            .get("instructions")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let effective_instructions = response_instructions.or_else(|| self.instructions.clone());
        if let Some(instructions) = effective_instructions.clone() {
            if !instructions.trim().is_empty() {
                messages.push(CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text { text: instructions }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                });
            }
        }
        messages.extend(self.messages.clone());
        if messages
            .iter()
            .all(|message| !matches!(message.role, MessageRole::User))
        {
            if let Some(prompt) = effective_instructions {
                if !prompt.trim().is_empty() {
                    messages.push(CanonicalMessage {
                        // Some upstream families require at least one user turn even when
                        // OpenAI Realtime callers only provide `response.instructions`.
                        role: MessageRole::User,
                        content: vec![ContentPart::Text { text: prompt }],
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![],
                    });
                }
            }
        }

        let (tools, tool_choice) = parse_openai_session_tools(response)
            .unwrap_or_else(|| (self.tools.clone(), self.tool_choice.clone()));
        let requested_model = response
            .get("model")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| self.model.clone());

        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAiRealtime,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(requested_model),
            stream: true,
            messages,
            tools,
            tool_choice,
            reasoning: None,
            metadata: None,
            raw_body: response.clone(),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }
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

async fn run_realtime_response(
    socket: &mut WebSocket,
    state: &Arc<AppState>,
    token: &Option<String>,
    headers: &HeaderMap,
    query_params: &HashMap<String, String>,
    session: &mut RealtimeSession,
    response: Option<&Value>,
) -> Result<(), GatewayError> {
    let response_id = format!("resp_{}", uuid::Uuid::new_v4());
    let message_item_id = format!("item_{}", uuid::Uuid::new_v4());
    let request = session.build_request(response);
    let request_model = request
        .requested_model
        .clone()
        .unwrap_or_else(|| session.model.clone());
    let request_messages_text = request.messages_text();
    let request_tools = request.tools.clone();
    let request_tool_choice = request.tool_choice.clone();
    let mut ctx = PipelineContext::new(request, token.clone());
    apply_public_request_headers(&mut ctx, headers, &state.config, &[]);
    ctx.query_params = query_params.clone();

    send_json(
        socket,
        json!({
            "type": "response.created",
            "response": {
                "id": response_id,
                "status": "in_progress",
                "model": request_model,
            }
        }),
    )
    .await?;
    send_json(
        socket,
        json!({
            "type": "response.output_item.added",
            "response_id": response_id,
            "output_index": 0,
            "item": {
                "id": message_item_id,
                "type": "message",
                "role": "assistant",
                "status": "in_progress",
                "content": [],
            }
        }),
    )
    .await?;

    let mut assistant_text = String::new();
    let mut usage = None;
    let mut finish_reason = None;
    let mut pending_tools: HashMap<usize, PendingToolCall> = HashMap::new();
    let mut accumulator = OpenAiSseAccumulator::default();

    match run_pipeline(ctx, state).await? {
        PipelineOutput::Sse(mut stream) => {
            while let Some(chunk) = stream.next().await {
                let bytes = chunk.map_err(|error| {
                    GatewayError::server_error(format!(
                        "realtime bridge upstream stream failed: {error}"
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
                                "type": "response.text.delta",
                                "response_id": response_id,
                                "item_id": message_item_id,
                                "output_index": 0,
                                "content_index": 0,
                                "delta": delta,
                            }),
                        )
                        .await?;
                    }

                    for delta in &frame.tool_call_deltas {
                        let entry = pending_tools.entry(delta.index).or_insert_with(|| {
                            PendingToolCall::new(format!("item_{}", uuid::Uuid::new_v4()))
                        });
                        let before_name = entry.name.clone();
                        let arguments_delta = entry.apply_delta(delta);
                        if before_name.is_empty() && !entry.name.is_empty() {
                            send_json(
                                socket,
                                json!({
                                    "type": "response.output_item.added",
                                    "response_id": response_id,
                                    "output_index": 0,
                                    "item": {
                                        "id": entry.item_id,
                                        "type": "function_call",
                                        "status": "in_progress",
                                        "call_id": entry.id,
                                        "name": entry.name,
                                        "arguments": entry.arguments,
                                    }
                                }),
                            )
                            .await?;
                        }
                        if let Some(arguments_delta) = arguments_delta {
                            send_json(
                                socket,
                                json!({
                                    "type": "response.function_call_arguments.delta",
                                    "response_id": response_id,
                                    "item_id": entry.item_id,
                                    "output_index": 0,
                                    "call_id": entry.id,
                                    "delta": arguments_delta,
                                }),
                            )
                            .await?;
                        }
                    }

                    if frame.usage.is_some() {
                        usage = frame.usage;
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
            usage = canonical_resp.usage;
            finish_reason = canonical_resp.finish_reason;
            for (index, tool_call) in canonical_resp.tool_calls.iter().enumerate() {
                pending_tools.insert(
                    index,
                    PendingToolCall {
                        id: tool_call
                            .id
                            .clone()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                        item_id: format!("item_{}", uuid::Uuid::new_v4()),
                        name: tool_call.name.clone().unwrap_or_default(),
                        arguments: tool_call.arguments.clone().unwrap_or_default(),
                    },
                );
            }
        }
        PipelineOutput::Binary(_) => {
            return Err(GatewayError::server_error(
                "unexpected binary response for realtime bridge",
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
                        item_id: format!("item_{}", uuid::Uuid::new_v4()),
                        name: tool_call.name.clone().unwrap_or_default(),
                        arguments: tool_call.arguments.clone().unwrap_or_default(),
                    },
                );
            }
        }
    }

    send_json(
        socket,
        json!({
            "type": "response.output_item.done",
            "response_id": response_id,
            "output_index": 0,
            "item": {
                "id": message_item_id,
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": if assistant_text.is_empty() {
                    Vec::<Value>::new()
                } else {
                    vec![json!({"type": "text", "text": assistant_text})]
                },
            }
        }),
    )
    .await?;

    let mut output_items = Vec::new();
    if !assistant_text.is_empty() {
        output_items.push(json!({
            "id": message_item_id,
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "text", "text": assistant_text}],
        }));
    }

    for tool_call in pending_tools.values() {
        send_json(
            socket,
            json!({
                "type": "response.output_item.done",
                "response_id": response_id,
                "output_index": 0,
                "item": {
                    "id": tool_call.item_id,
                    "type": "function_call",
                    "status": "completed",
                    "call_id": tool_call.id,
                    "name": tool_call.name,
                    "arguments": tool_call.arguments,
                }
            }),
        )
        .await?;
        output_items.push(json!({
            "id": tool_call.item_id,
            "type": "function_call",
            "status": "completed",
            "call_id": tool_call.id,
            "name": tool_call.name,
            "arguments": tool_call.arguments,
        }));
    }

    send_json(
        socket,
        json!({
            "type": "response.done",
            "response": {
                "id": response_id,
                "status": "completed",
                "output": output_items,
                "usage": usage.as_ref().map(|usage| json!({
                    "input_tokens": usage.prompt_tokens,
                    "output_tokens": usage.completion_tokens,
                    "total_tokens": usage.total_tokens,
                })),
                "status_details": finish_reason,
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

fn parse_openai_session_tools(
    raw: &Value,
) -> Option<(
    Vec<crate::protocol::canonical::CanonicalTool>,
    Option<Value>,
)> {
    if raw.get("tools").is_none() && raw.get("tool_choice").is_none() {
        return None;
    }
    let prefers_responses_shape = raw
        .get("tools")
        .and_then(|value| value.as_array())
        .map(|tools| {
            tools.iter().any(|tool| {
                tool.get("function").is_none()
                    && tool
                        .get("type")
                        .and_then(|value| value.as_str())
                        .map(|value| value.eq_ignore_ascii_case("function"))
                        .unwrap_or(false)
                    && (tool.get("name").is_some()
                        || tool.get("description").is_some()
                        || tool.get("parameters").is_some())
            })
        })
        .unwrap_or(false);
    if prefers_responses_shape {
        let responses_synthetic = json!({
            "model": raw.get("model").cloned().unwrap_or_else(|| json!("gpt-5.4")),
            "input": [],
            "tools": raw.get("tools").cloned().unwrap_or_else(|| json!([])),
            "tool_choice": raw.get("tool_choice").cloned().unwrap_or(Value::Null),
        });
        if let Ok(request) = responses::normalize_responses(responses_synthetic) {
            return Some((request.tools, request.tool_choice));
        }
    }
    let chat_synthetic = json!({
        "model": raw.get("model").cloned().unwrap_or_else(|| json!("gpt-5.4")),
        "messages": [],
        "tools": raw.get("tools").cloned().unwrap_or_else(|| json!([])),
        "tool_choice": raw.get("tool_choice").cloned().unwrap_or(Value::Null),
    });
    if let Ok(request) = openai::normalize_chat_completions(chat_synthetic) {
        return Some((request.tools, request.tool_choice));
    }
    let responses_synthetic = json!({
        "model": raw.get("model").cloned().unwrap_or_else(|| json!("gpt-5.4")),
        "input": [],
        "tools": raw.get("tools").cloned().unwrap_or_else(|| json!([])),
        "tool_choice": raw.get("tool_choice").cloned().unwrap_or(Value::Null),
    });
    let request = responses::normalize_responses(responses_synthetic).ok()?;
    Some((request.tools, request.tool_choice))
}

fn normalize_realtime_item(item: &Value) -> Option<CanonicalMessage> {
    match item
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("message")
    {
        "message" => {
            let role = match item
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or("user")
            {
                "assistant" => MessageRole::Assistant,
                "system" => MessageRole::System,
                _ => MessageRole::User,
            };
            let text = item
                .get("content")
                .and_then(|value| value.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| {
                            part.get("text")
                                .or_else(|| part.get("input_text"))
                                .and_then(|value| value.as_str())
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            Some(CanonicalMessage {
                role,
                content: if text.is_empty() {
                    Vec::new()
                } else {
                    vec![ContentPart::Text { text }]
                },
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            })
        }
        "function_call_output" => {
            let output = item.get("output").cloned().unwrap_or_else(|| json!(""));
            let content = if let Some(text) = output.as_str() {
                if let Ok(value) = serde_json::from_str::<Value>(text) {
                    vec![ContentPart::Json { value }]
                } else {
                    vec![ContentPart::Text {
                        text: text.to_string(),
                    }]
                }
            } else {
                vec![ContentPart::Json { value: output }]
            };
            Some(CanonicalMessage {
                role: MessageRole::Tool,
                content,
                name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_call_id: item
                    .get("call_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_calls: Vec::new(),
            })
        }
        _ => None,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_openai_session_tools;

    #[test]
    fn parse_openai_session_tools_accepts_responses_function_shape() {
        let raw = json!({
            "model": "gpt-5.4",
            "tools": [
                {
                    "type": "function",
                    "name": "weather",
                    "description": "Return the weather for a city.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "city": { "type": "string" }
                        },
                        "required": ["city"]
                    }
                }
            ],
            "tool_choice": "required"
        });

        let (tools, tool_choice) = parse_openai_session_tools(&raw).expect("tools should parse");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_deref(), Some("weather"));
        assert_eq!(tool_choice, Some(json!("required")));
    }
}
