use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::sse_parse::{parse_sse_line, SseFrame, SseParseState};

use super::parse_json_or_string;

pub fn summarize_conversation_stream_text(raw_text: &str) -> Result<Value, GatewayError> {
    let mut parser_state = SseParseState::new();
    let mut conversation_id: Option<String> = None;
    let mut tool_calls = Vec::new();
    let mut tool_returns = Vec::new();
    let mut retry_prompts = Vec::new();
    let mut suggestions = Vec::new();
    let mut message_texts = Vec::new();
    let mut final_event_seen = false;

    for raw_line in raw_text.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if let Some(frame) = parse_sse_line(line, &mut parser_state) {
            handle_conversation_summary_frame(
                &frame,
                &mut conversation_id,
                &mut tool_calls,
                &mut tool_returns,
                &mut retry_prompts,
                &mut suggestions,
                &mut message_texts,
                &mut final_event_seen,
            )?;
        }
    }

    if let Some(frame) = parse_sse_line("", &mut parser_state) {
        handle_conversation_summary_frame(
            &frame,
            &mut conversation_id,
            &mut tool_calls,
            &mut tool_returns,
            &mut retry_prompts,
            &mut suggestions,
            &mut message_texts,
            &mut final_event_seen,
        )?;
    }

    Ok(json!({
        "conversation_id": conversation_id,
        "tool_calls": tool_calls,
        "tool_returns": tool_returns,
        "retry_prompts": retry_prompts,
        "suggestions": suggestions,
        "message_texts": message_texts,
        "final_event_seen": final_event_seen,
    }))
}

fn handle_conversation_summary_frame(
    frame: &SseFrame,
    conversation_id: &mut Option<String>,
    tool_calls: &mut Vec<Value>,
    tool_returns: &mut Vec<Value>,
    retry_prompts: &mut Vec<String>,
    suggestions: &mut Vec<String>,
    message_texts: &mut Vec<String>,
    final_event_seen: &mut bool,
) -> Result<(), GatewayError> {
    let event_name = frame.event_name.clone().unwrap_or_default();
    let data = parse_json_or_string(&frame.data);

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
        "part" => {
            let Some(part) = data
                .as_object()
                .and_then(|value| value.get("part"))
                .and_then(|value| value.as_object())
            else {
                return Ok(());
            };
            let part_kind = part
                .get("part_kind")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .unwrap_or_default();
            match part_kind {
                "tool-call" => tool_calls.push(Value::Object(part.clone())),
                "tool-return" => tool_returns.push(Value::Object(part.clone())),
                "retry-prompt" => {
                    if let Some(text) = part
                        .get("content")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        retry_prompts.push(text.to_string());
                    }
                }
                "text" => {
                    if let Some(text) = part
                        .get("content")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        message_texts.push(text.to_string());
                    }
                }
                _ => {}
            }
        }
        "suggestion" => {
            if let Some(parts) = data
                .as_object()
                .and_then(|value| value.get("parts"))
                .and_then(|value| value.as_array())
            {
                for part in parts {
                    let Some(part) = part.as_object() else {
                        continue;
                    };
                    let part_kind = part
                        .get("part_kind")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .unwrap_or_default();
                    if part_kind == "text" {
                        if let Some(text) = part
                            .get("content")
                            .and_then(|value| value.as_str())
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            message_texts.push(text.to_string());
                        }
                    }
                    if part_kind == "tool-call"
                        && part
                            .get("tool_name")
                            .and_then(|value| value.as_str())
                            .map(str::trim)
                            == Some("synthetic__suggest_actions")
                    {
                        if let Some(args) = part.get("args").and_then(|value| value.as_object()) {
                            for value in args.values() {
                                if let Some(text) = value
                                    .as_str()
                                    .map(str::trim)
                                    .filter(|entry| !entry.is_empty())
                                {
                                    suggestions.push(text.to_string());
                                }
                            }
                        }
                    }
                }
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
                .unwrap_or("Producer.ai conversation stream returned an error event");
            return Err(GatewayError::server_error(message)
                .with_provider("producer_compatible")
                .with_code("producer_stream_error_event"));
        }
        _ => {}
    }

    Ok(())
}
