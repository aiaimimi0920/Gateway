use std::collections::BTreeMap;

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

use super::unpack_responses_response;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall};
use crate::protocol::sse_parse::{parse_sse_line as parse_sse_frame_line, SseFrame, SseParseState};

const MAX_RESPONSES_SSE_ACCUMULATION_BYTES: usize = 64 * 1024 * 1024;

pub async fn accumulate_responses_stream(
    response: rquest::Response,
    fallback_model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSES_SSE_ACCUMULATION_BYTES as u64)
    {
        return Err(responses_stream_too_large_error(
            MAX_RESPONSES_SSE_ACCUMULATION_BYTES,
        ));
    }
    accumulate_responses_sse_stream(Box::pin(response.bytes_stream()), fallback_model).await
}

pub async fn accumulate_responses_sse_stream(
    stream: std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    fallback_model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    accumulate_responses_sse_stream_with_limit(
        stream,
        fallback_model,
        MAX_RESPONSES_SSE_ACCUMULATION_BYTES,
    )
    .await
}

pub(super) async fn accumulate_responses_sse_stream_with_limit(
    mut stream: std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    fallback_model: &str,
    max_bytes: usize,
) -> Result<CanonicalRelayResponse, GatewayError> {
    use futures::StreamExt;

    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            GatewayError::server_error(format!(
                "failed to read translated responses SSE chunk: {error}"
            ))
        })?;
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .filter(|length| *length <= max_bytes)
            .ok_or_else(|| responses_stream_too_large_error(max_bytes))?;
        body.try_reserve(next_len - body.len()).map_err(|error| {
            GatewayError::server_error(format!(
                "failed to reserve responses SSE accumulation buffer: {error}"
            ))
            .with_code("responses_stream_buffer_allocation_failed")
        })?;
        body.extend_from_slice(&chunk);
    }
    accumulate_responses_sse_bytes(&body, fallback_model)
}

fn responses_stream_too_large_error(max_bytes: usize) -> GatewayError {
    GatewayError::server_error(format!(
        "Responses upstream SSE body exceeded the {max_bytes}-byte accumulation limit."
    ))
    .with_code("responses_stream_too_large")
}

pub(super) fn accumulate_responses_sse_bytes(
    body: &[u8],
    fallback_model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let text = String::from_utf8_lossy(body);
    let mut parser = SseParseState::new();
    let mut collected_text = String::new();
    let mut completed_response: Option<Value> = None;
    let mut streamed_tool_calls = BTreeMap::<usize, PendingAccumulatedToolCall>::new();

    let mut handle_frame = |frame: SseFrame| -> Result<(), GatewayError> {
        if frame.data.is_empty() || frame.data == "[DONE]" {
            return Ok(());
        }

        let payload: Value = serde_json::from_str(&frame.data).map_err(|error| {
            GatewayError::server_error(format!("failed to parse responses SSE frame: {error}"))
                .with_code("responses_stream_parse_failed")
        })?;
        let event_type = frame
            .event_name
            .as_deref()
            .or_else(|| payload.get("type").and_then(|value| value.as_str()))
            .unwrap_or_default();

        match event_type {
            "error" => {
                return Err(GatewayError::server_error(format!(
                    "responses upstream stream error: {}",
                    frame.data
                ))
                .with_code("responses_stream_upstream_error"));
            }
            "response.output_text.delta" => {
                if let Some(delta) = payload.get("delta").and_then(|value| value.as_str()) {
                    collected_text.push_str(delta);
                }
            }
            "response.output_item.added" | "response.output_item.done" => {
                if let Some(item) = payload.get("item") {
                    collect_streamed_tool_call_item(
                        item,
                        payload.get("output_index").and_then(|value| value.as_u64()),
                        &mut streamed_tool_calls,
                    );
                }
            }
            "response.function_call_arguments.delta" => {
                if let Some(item_id) = payload.get("item_id").and_then(|value| value.as_str()) {
                    let output_index = payload
                        .get("output_index")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as usize)
                        .or_else(|| {
                            streamed_tool_calls.iter().find_map(|(index, call)| {
                                if call.item_id == item_id {
                                    Some(*index)
                                } else {
                                    None
                                }
                            })
                        });
                    if let (Some(output_index), Some(delta)) = (
                        output_index,
                        payload.get("delta").and_then(|value| value.as_str()),
                    ) {
                        let entry = streamed_tool_calls.entry(output_index).or_insert_with(|| {
                            PendingAccumulatedToolCall {
                                item_id: item_id.to_string(),
                                call_id: item_id.to_string(),
                                name: None,
                                arguments: String::new(),
                            }
                        });
                        entry.arguments.push_str(delta);
                    }
                }
            }
            "response.function_call_arguments.done" => {
                if let Some(item_id) = payload.get("item_id").and_then(|value| value.as_str()) {
                    let output_index = payload
                        .get("output_index")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as usize)
                        .or_else(|| {
                            streamed_tool_calls.iter().find_map(|(index, call)| {
                                if call.item_id == item_id {
                                    Some(*index)
                                } else {
                                    None
                                }
                            })
                        });
                    if let Some(output_index) = output_index {
                        let entry = streamed_tool_calls.entry(output_index).or_insert_with(|| {
                            PendingAccumulatedToolCall {
                                item_id: item_id.to_string(),
                                call_id: item_id.to_string(),
                                name: None,
                                arguments: String::new(),
                            }
                        });
                        if let Some(arguments) =
                            payload.get("arguments").and_then(|value| value.as_str())
                        {
                            entry.arguments = arguments.to_string();
                        }
                    }
                }
            }
            "response.completed" => {
                if let Some(response) = payload.get("response") {
                    completed_response = Some(response.clone());
                }
            }
            _ => {}
        }

        Ok(())
    };

    for line in text.split('\n') {
        if let Some(frame) = parse_sse_frame_line(line, &mut parser) {
            handle_frame(frame)?;
        }
    }
    if let Some(frame) = parse_sse_frame_line("", &mut parser) {
        handle_frame(frame)?;
    }

    if let Some(response_body) = completed_response {
        let mut canonical = unpack_responses_response(&response_body)?;
        if canonical.model == "unknown" {
            canonical.model = fallback_model.to_string();
        }
        if canonical.text.is_empty() && !collected_text.is_empty() {
            canonical.text = collected_text;
        }
        if canonical.tool_calls.is_empty() && !streamed_tool_calls.is_empty() {
            canonical.tool_calls = streamed_tool_calls
                .into_values()
                .map(|call| CanonicalToolCall {
                    id: Some(call.call_id),
                    call_type: "function".to_string(),
                    name: call.name,
                    arguments: Some(crate::protocol::accio::normalize_tool_args(&call.arguments)),
                    raw: std::collections::HashMap::new(),
                })
                .collect();
        }
        if !canonical.tool_calls.is_empty() {
            canonical.finish_reason = Some("tool_calls".to_string());
        }
        return Ok(canonical);
    }

    if !collected_text.is_empty() || !streamed_tool_calls.is_empty() {
        let has_text = !collected_text.is_empty();
        return Ok(CanonicalRelayResponse {
            model: fallback_model.to_string(),
            text: collected_text,
            usage: None,
            tool_calls: streamed_tool_calls
                .into_values()
                .map(|call| CanonicalToolCall {
                    id: Some(call.call_id),
                    call_type: "function".to_string(),
                    name: call.name,
                    arguments: Some(crate::protocol::accio::normalize_tool_args(&call.arguments)),
                    raw: std::collections::HashMap::new(),
                })
                .collect(),
            upstream_status: Some(200),
            finish_reason: Some(if has_text {
                "stop".to_string()
            } else {
                "tool_calls".to_string()
            }),
        });
    }

    Err(GatewayError::server_error(
        "Responses stream ended without a completed frame or text payload.",
    )
    .with_code("responses_stream_incomplete"))
}

#[derive(Debug, Clone)]
struct PendingAccumulatedToolCall {
    item_id: String,
    call_id: String,
    name: Option<String>,
    arguments: String,
}

fn collect_streamed_tool_call_item(
    item: &Value,
    output_index: Option<u64>,
    streamed_tool_calls: &mut BTreeMap<usize, PendingAccumulatedToolCall>,
) {
    let item_type = item.get("type").and_then(|value| value.as_str());
    if !matches!(item_type, Some("function_call") | Some("custom_tool_call")) {
        return;
    }

    let Some(output_index) = output_index.map(|value| value as usize) else {
        return;
    };

    let entry =
        streamed_tool_calls
            .entry(output_index)
            .or_insert_with(|| PendingAccumulatedToolCall {
                item_id: item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                call_id: item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: None,
                arguments: String::new(),
            });

    if let Some(item_id) = item.get("id").and_then(|value| value.as_str()) {
        if !item_id.trim().is_empty() {
            entry.item_id = item_id.to_string();
        }
    }
    if let Some(call_id) = item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(|value| value.as_str())
    {
        if !call_id.trim().is_empty() {
            entry.call_id = call_id.to_string();
        }
    }
    if let Some(name) = item.get("name").and_then(|value| value.as_str()) {
        if !name.trim().is_empty() {
            entry.name = Some(name.to_string());
        }
    }
    if let Some(arguments) = item.get("arguments").and_then(|value| value.as_str()) {
        if !arguments.is_empty() {
            entry.arguments = arguments.to_string();
        }
    }
}
