use serde_json::{json, Value};

use super::parse_json_or_string;
use crate::error::GatewayError;
use crate::protocol::sse_parse::{parse_sse_line, SseFrame, SseParseState};
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider;

pub fn extract_job_id(body: &Value) -> Result<String, GatewayError> {
    body.get("job_id")
        .or_else(|| body.get("jobId"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error("Producer.ai send-message response missing job_id")
                .with_provider("producer_compatible")
                .with_code("producer_missing_job_id")
        })
}

pub fn extract_stream_job_id(body: &Value) -> Result<String, GatewayError> {
    extract_job_id(body).map_err(|error| error.with_code("producer_missing_stream_job_id"))
}

pub async fn accumulate_job_stream(
    response: rquest::Response,
    model: &str,
    job_id: &str,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let raw_text = collect_bounded_upstream_text_with_provider(
        response,
        "Producer.ai music SSE body",
        provider,
    )
    .await?;

    parse_producer_music_stream_text(&raw_text, model, job_id)
}

pub fn parse_producer_music_stream_text(
    raw_text: &str,
    model: &str,
    job_id: &str,
) -> Result<Value, GatewayError> {
    let mut parser_state = SseParseState::new();
    let mut events = Vec::new();
    let mut parts = Vec::new();
    let mut suggestions = Vec::new();
    let mut conversation_id: Option<String> = None;
    let mut generated_title: Option<String> = None;
    let mut completed = false;
    let mut final_event_seen = false;

    for raw_line in raw_text.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if let Some(frame) = parse_sse_line(line, &mut parser_state) {
            handle_sse_frame(
                &frame,
                &mut events,
                &mut parts,
                &mut suggestions,
                &mut conversation_id,
                &mut generated_title,
                &mut completed,
                &mut final_event_seen,
            )?;
        }
    }

    if let Some(frame) = parse_sse_line("", &mut parser_state) {
        handle_sse_frame(
            &frame,
            &mut events,
            &mut parts,
            &mut suggestions,
            &mut conversation_id,
            &mut generated_title,
            &mut completed,
            &mut final_event_seen,
        )?;
    }

    Ok(json!({
        "object": "music.generation",
        "provider": "producer.ai",
        "model": model,
        "job_id": job_id,
        "conversation_id": conversation_id,
        "generated_title": generated_title,
        "completed": completed,
        "final_event_seen": final_event_seen,
        "parts": parts,
        "suggestions": suggestions,
        "events": events,
    }))
}

fn handle_sse_frame(
    frame: &SseFrame,
    events: &mut Vec<Value>,
    parts: &mut Vec<Value>,
    suggestions: &mut Vec<Value>,
    conversation_id: &mut Option<String>,
    generated_title: &mut Option<String>,
    completed: &mut bool,
    final_event_seen: &mut bool,
) -> Result<(), GatewayError> {
    let event_name = frame.event_name.clone().unwrap_or_default();
    let data = parse_json_or_string(&frame.data);

    events.push(json!({
        "event": if event_name.is_empty() { Value::Null } else { Value::String(event_name.clone()) },
        "data": data.clone(),
    }));

    match event_name.as_str() {
        "conversation_id" => {
            if let Some(id) = data
                .as_object()
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                *conversation_id = Some(id.to_string());
            }
        }
        "part" => parts.push(data),
        "suggestion" => suggestions.push(data),
        "generated-title" => {
            if let Some(title) = data
                .as_object()
                .and_then(|value| value.get("title"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                *generated_title = Some(title.to_string());
            }
        }
        "complete" => *completed = true,
        "final" => *final_event_seen = true,
        "error" => {
            let message = data
                .as_object()
                .and_then(|value| value.get("message"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Producer.ai stream returned an error event");
            return Err(GatewayError::server_error(message)
                .with_provider("producer_compatible")
                .with_code("producer_stream_error_event"));
        }
        _ => {}
    }

    Ok(())
}
