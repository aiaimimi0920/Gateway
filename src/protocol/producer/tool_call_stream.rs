use serde_json::{json, Map, Value};

use crate::error::GatewayError;
use crate::protocol::sse_parse::{parse_sse_line, SseFrame, SseParseState};
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider;

use super::{
    build_session_url, build_video_detail_path, build_video_status_path, parse_json_or_string,
};

pub async fn accumulate_tool_call_stream(
    response: rquest::Response,
    model: &str,
    stream_job_id: &str,
    tool_name: &str,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let buffer = collect_bounded_upstream_text_with_provider(
        response,
        "Producer.ai tool-call SSE body",
        provider,
    )
    .await?;

    parse_tool_call_stream_text(&buffer, model, stream_job_id, tool_name)
}

pub(super) fn parse_tool_call_stream_text(
    raw_text: &str,
    model: &str,
    stream_job_id: &str,
    tool_name: &str,
) -> Result<Value, GatewayError> {
    let mut parser_state = SseParseState::new();
    let mut events = Vec::new();
    let mut messages = Vec::new();
    let mut tool_returns = Vec::new();
    let mut video_urls = Vec::new();
    let mut conversation_id: Option<String> = None;
    let mut tool_return_job_id: Option<String> = None;
    let mut final_event_seen = false;

    for raw_line in raw_text.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if let Some(frame) = parse_sse_line(line, &mut parser_state) {
            handle_tool_call_sse_frame(
                &frame,
                tool_name,
                &mut events,
                &mut messages,
                &mut tool_returns,
                &mut video_urls,
                &mut conversation_id,
                &mut tool_return_job_id,
                &mut final_event_seen,
            )?;
        }
    }

    if let Some(frame) = parse_sse_line("", &mut parser_state) {
        handle_tool_call_sse_frame(
            &frame,
            tool_name,
            &mut events,
            &mut messages,
            &mut tool_returns,
            &mut video_urls,
            &mut conversation_id,
            &mut tool_return_job_id,
            &mut final_event_seen,
        )?;
    }

    let job_id = tool_return_job_id
        .clone()
        .unwrap_or_else(|| stream_job_id.to_string());
    let progress_url = conversation_id
        .as_ref()
        .map(|id| build_session_url("https://www.producer.ai", id));

    Ok(json!({
        "object": "video.generation",
        "provider": "producer.ai",
        "model": model,
        "tool_name": tool_name,
        "job_id": job_id,
        "stream_job_id": stream_job_id,
        "tool_return_job_id": tool_return_job_id,
        "conversation_id": conversation_id,
        "progress_url": progress_url,
        "status_path": build_video_status_path(&job_id),
        "detail_path": build_video_detail_path(&job_id),
        "library_path": "/__api/music-videos/get",
        "video_urls": video_urls,
        "tool_returns": tool_returns,
        "messages": messages,
        "final_event_seen": final_event_seen,
        "events": events,
    }))
}

fn handle_tool_call_sse_frame(
    frame: &SseFrame,
    tool_name: &str,
    events: &mut Vec<Value>,
    messages: &mut Vec<Value>,
    tool_returns: &mut Vec<Value>,
    video_urls: &mut Vec<String>,
    conversation_id: &mut Option<String>,
    tool_return_job_id: &mut Option<String>,
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
        "message" => {
            messages.push(data.clone());
            for content in extract_tool_return_contents(&data, tool_name) {
                if let Some(job_id) = content
                    .get("job_id")
                    .or_else(|| content.get("jobId"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    *tool_return_job_id = Some(job_id.to_string());
                }
                if let Some(url) = content
                    .get("url")
                    .or_else(|| content.get("final_video_url"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    video_urls.push(url.to_string());
                }
                tool_returns.push(Value::Object(content));
            }
        }
        "final" => *final_event_seen = true,
        "error" => {
            let message = data
                .as_object()
                .and_then(|value| value.get("message"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Producer.ai tool-call stream returned an error event");
            return Err(GatewayError::server_error(message)
                .with_provider("producer_compatible")
                .with_code("producer_tool_call_stream_error_event"));
        }
        _ => {}
    }

    Ok(())
}

fn extract_tool_return_contents(data: &Value, tool_name: &str) -> Vec<Map<String, Value>> {
    let Some(parts) = data
        .as_object()
        .and_then(|value| value.get("parts"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    parts
        .iter()
        .filter_map(|part| {
            let part_map = part.as_object()?;
            if part_map
                .get("part_kind")
                .and_then(|value| value.as_str())
                .map(str::trim)
                != Some("tool-return")
            {
                return None;
            }

            let matches_tool = part_map
                .get("tool_name")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .map(|value| value == tool_name)
                .unwrap_or(true);
            if !matches_tool {
                return None;
            }

            part_map
                .get("content")
                .and_then(|value| value.as_object())
                .cloned()
        })
        .collect()
}
