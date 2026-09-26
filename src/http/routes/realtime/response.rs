//! Realtime pipeline execution, streaming deltas and response completion.

use super::super::ws_bridge::{OpenAiSseAccumulator, PendingToolCall};
use super::send_json;
use super::session::RealtimeSession;
use crate::error::GatewayError;
use crate::http::request_headers::apply_public_request_headers;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::canonical::CanonicalMessage;
use crate::protocol::canonical::ContentPart;
use crate::protocol::canonical::MessageRole;
use crate::protocol::openai;
use crate::protocol::tool_inject;
use crate::state::AppState;
use axum::extract::ws::WebSocket;
use axum::http::HeaderMap;
use futures::StreamExt;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) async fn run_realtime_response(
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
    apply_public_request_headers(
        &mut ctx,
        headers,
        &state.config,
        state.console_auth.as_ref(),
        &[],
    )?;
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
