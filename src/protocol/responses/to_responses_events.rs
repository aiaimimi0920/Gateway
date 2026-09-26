use serde_json::{json, Value};

use crate::protocol::sse_parse::format_sse_event;

pub(super) fn build_response_created_event(
    sequence_number: u64,
    response_id: &str,
    model: &str,
    created_at: i64,
) -> String {
    let payload = json!({
        "type": "response.created",
        "sequence_number": sequence_number,
        "response": {
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "model": model,
            "status": "in_progress",
            "output": [],
        }
    });
    format_sse_event(Some("response.created"), &payload.to_string())
}

pub(super) fn build_response_in_progress_event(
    sequence_number: u64,
    response_id: &str,
    model: &str,
    created_at: i64,
) -> String {
    let payload = json!({
        "type": "response.in_progress",
        "sequence_number": sequence_number,
        "response": {
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "model": model,
            "status": "in_progress",
        }
    });
    format_sse_event(Some("response.in_progress"), &payload.to_string())
}

pub(super) fn build_response_message_item_added(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.added",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "message",
            "status": "in_progress",
            "role": "assistant",
            "content": [],
        }
    });
    format_sse_event(Some("response.output_item.added"), &payload.to_string())
}

pub(super) fn build_response_content_part_added(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
) -> String {
    let payload = json!({
        "type": "response.content_part.added",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "part": {
            "type": "output_text",
            "text": "",
        }
    });
    format_sse_event(Some("response.content_part.added"), &payload.to_string())
}

pub(super) fn build_response_output_text_delta(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    delta: &str,
) -> String {
    let payload = json!({
        "type": "response.output_text.delta",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "delta": delta,
    });
    format_sse_event(Some("response.output_text.delta"), &payload.to_string())
}

pub(super) fn build_response_output_text_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    text: &str,
) -> String {
    let payload = json!({
        "type": "response.output_text.done",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "text": text,
    });
    format_sse_event(Some("response.output_text.done"), &payload.to_string())
}

pub(super) fn build_response_content_part_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    text: &str,
) -> String {
    let payload = json!({
        "type": "response.content_part.done",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "part": {
            "type": "output_text",
            "text": text,
        }
    });
    format_sse_event(Some("response.content_part.done"), &payload.to_string())
}

pub(super) fn build_response_message_item_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    text: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.done",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "message",
            "status": "completed",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": text,
            }],
        }
    });
    format_sse_event(Some("response.output_item.done"), &payload.to_string())
}

pub(super) fn build_response_function_call_item_added(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    call_id: &str,
    name: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.added",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "function_call",
            "status": "in_progress",
            "call_id": call_id,
            "name": name,
            "arguments": "",
        }
    });
    format_sse_event(Some("response.output_item.added"), &payload.to_string())
}

pub(super) fn build_response_function_call_arguments_delta(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    delta: &str,
) -> String {
    let payload = json!({
        "type": "response.function_call_arguments.delta",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item_id": item_id,
        "delta": delta,
    });
    format_sse_event(
        Some("response.function_call_arguments.delta"),
        &payload.to_string(),
    )
}

pub(super) fn build_response_function_call_arguments_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    arguments: &str,
) -> String {
    let payload = json!({
        "type": "response.function_call_arguments.done",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item_id": item_id,
        "arguments": arguments,
    });
    format_sse_event(
        Some("response.function_call_arguments.done"),
        &payload.to_string(),
    )
}

pub(super) fn build_response_function_call_item_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    call_id: &str,
    name: &str,
    arguments: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.done",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "function_call",
            "status": "completed",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        }
    });
    format_sse_event(Some("response.output_item.done"), &payload.to_string())
}

pub(super) fn build_response_completed_event(sequence_number: u64, response: Value) -> String {
    let payload = json!({
        "type": "response.completed",
        "sequence_number": sequence_number,
        "response": response,
    });
    format_sse_event(Some("response.completed"), &payload.to_string())
}
