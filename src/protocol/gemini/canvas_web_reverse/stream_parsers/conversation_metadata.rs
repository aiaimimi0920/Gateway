//! Recover conversation locators and the newest entry for each conversation.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas as legacy;

use super::wire_frames::{parse_stream_frames_or_envelopes, strip_xssi_prefix};

pub fn extract_stream_generate_locator(
    body: &str,
) -> Result<legacy::GeminiCanvasStreamGenerateLocator, GatewayError> {
    let normalized = strip_xssi_prefix(body);
    let frames = parse_stream_frames_or_envelopes(normalized);

    let mut response_id = None;
    let mut conversation_id = None;
    for frame in &frames {
        collect_stream_generate_locator(frame, &mut response_id, &mut conversation_id);
        if response_id.is_some() && conversation_id.is_some() {
            break;
        }
    }
    if response_id.is_none() {
        response_id = scan_stream_generate_id_in_text(normalized, "r_");
    }
    if conversation_id.is_none() {
        conversation_id = scan_stream_generate_id_in_text(normalized, "c_");
    }

    let response_id = response_id.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas StreamGenerate response did not expose a response id for TTS replay.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_missing_response_id")
    })?;
    let conversation_id = conversation_id.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas StreamGenerate response did not expose a conversation id for TTS replay.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_missing_conversation_id")
    })?;
    let app_conversation = conversation_id
        .strip_prefix("c_")
        .unwrap_or(conversation_id.as_str())
        .trim();
    if app_conversation.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas StreamGenerate conversation id could not be mapped to an app path.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_invalid_conversation_id"));
    }
    let app_path = format!("/app/{app_conversation}");

    Ok(legacy::GeminiCanvasStreamGenerateLocator {
        response_id,
        conversation_id,
        app_path,
    })
}

pub fn extract_stream_generate_response_id(body: &str) -> Result<String, GatewayError> {
    let normalized = strip_xssi_prefix(body);
    let frames = parse_stream_frames_or_envelopes(normalized);

    let mut response_id = None;
    let mut conversation_id = None;
    for frame in &frames {
        collect_stream_generate_locator(frame, &mut response_id, &mut conversation_id);
        if response_id.is_some() {
            break;
        }
    }
    if response_id.is_none() {
        response_id = scan_stream_generate_id_in_text(normalized, "r_");
    }

    response_id.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas StreamGenerate response did not expose a response id.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_missing_response_id")
    })
}

pub fn extract_conversation_list_entries(
    body: &str,
) -> Vec<legacy::GeminiCanvasConversationListEntry> {
    let normalized = strip_xssi_prefix(body);
    let frames = parse_stream_frames_or_envelopes(normalized);

    let mut collected = Vec::new();
    for frame in &frames {
        collect_conversation_list_entries(frame, &mut collected);
    }

    let mut deduped: HashMap<String, legacy::GeminiCanvasConversationListEntry> = HashMap::new();
    for entry in collected {
        let key = entry.conversation_id.clone();
        match deduped.get(&key) {
            Some(existing)
                if (existing.updated_at_secs, existing.updated_at_nanos)
                    >= (entry.updated_at_secs, entry.updated_at_nanos) => {}
            _ => {
                deduped.insert(key, entry);
            }
        }
    }

    let mut entries = deduped.into_values().collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        (right.updated_at_secs, right.updated_at_nanos)
            .cmp(&(left.updated_at_secs, left.updated_at_nanos))
    });
    entries
}

fn looks_like_stream_generate_id(value: &str, prefix: &str) -> bool {
    let candidate = value.trim();
    candidate.starts_with(prefix)
        && candidate.len() > prefix.len()
        && candidate[prefix.len()..]
            .chars()
            .all(|ch| ch.is_ascii_hexdigit())
}

fn scan_stream_generate_id_in_text(content: &str, prefix: &str) -> Option<String> {
    let body = content.trim();
    if body.is_empty() {
        return None;
    }

    let bytes = body.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    let mut index = 0usize;
    while index + prefix_bytes.len() <= bytes.len() {
        if &bytes[index..index + prefix_bytes.len()] == prefix_bytes {
            let mut end = index + prefix_bytes.len();
            while end < bytes.len() && (bytes[end] as char).is_ascii_hexdigit() {
                end += 1;
            }
            if end > index + prefix_bytes.len() {
                return Some(body[index..end].to_string());
            }
        }
        index += 1;
    }
    None
}

fn collect_stream_generate_locator(
    value: &Value,
    response_id: &mut Option<String>,
    conversation_id: &mut Option<String>,
) {
    if response_id.is_some() && conversation_id.is_some() {
        return;
    }

    match value {
        Value::Array(items) => {
            if conversation_id.is_none() {
                for pair in items.windows(2) {
                    let left = pair[0].as_str().map(str::trim);
                    let right = pair[1].as_str().map(str::trim);
                    if let (Some(left), Some(right)) = (left, right) {
                        if looks_like_stream_generate_id(left, "c_")
                            && looks_like_stream_generate_id(right, "r_")
                        {
                            *conversation_id = Some(left.to_string());
                            if response_id.is_none() {
                                *response_id = Some(right.to_string());
                            }
                            break;
                        }
                    }
                }
            }
            for item in items {
                collect_stream_generate_locator(item, response_id, conversation_id);
                if response_id.is_some() && conversation_id.is_some() {
                    return;
                }
            }
        }
        Value::Object(map) => {
            if response_id.is_none() {
                if let Some(value) = map.get("18").and_then(Value::as_str).map(str::trim) {
                    if looks_like_stream_generate_id(value, "r_") {
                        *response_id = Some(value.to_string());
                    }
                }
            }
            for item in map.values() {
                collect_stream_generate_locator(item, response_id, conversation_id);
                if response_id.is_some() && conversation_id.is_some() {
                    return;
                }
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if response_id.is_none() && looks_like_stream_generate_id(trimmed, "r_") {
                *response_id = Some(trimmed.to_string());
            }
            if conversation_id.is_none() && looks_like_stream_generate_id(trimmed, "c_") {
                *conversation_id = Some(trimmed.to_string());
            }
            if (response_id.is_none() || conversation_id.is_none())
                && (trimmed.starts_with('[') || trimmed.starts_with('{'))
            {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    collect_stream_generate_locator(&parsed, response_id, conversation_id);
                }
            }
        }
        _ => {}
    }
}

fn collect_conversation_list_entries(
    value: &Value,
    entries: &mut Vec<legacy::GeminiCanvasConversationListEntry>,
) {
    match value {
        Value::Array(items) => {
            if let Some(conversation_id) = items
                .first()
                .and_then(Value::as_str)
                .filter(|candidate| looks_like_stream_generate_id(candidate, "c_"))
            {
                let title = items
                    .get(1)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                let (updated_at_secs, updated_at_nanos) = items
                    .get(5)
                    .and_then(parse_conversation_list_timestamp)
                    .unwrap_or_default();
                let response_id = items
                    .get(6)
                    .and_then(extract_response_id_from_conversation_list_value);
                entries.push(legacy::GeminiCanvasConversationListEntry {
                    conversation_id: conversation_id.to_string(),
                    title,
                    response_id,
                    updated_at_secs,
                    updated_at_nanos,
                });
            }

            for item in items {
                collect_conversation_list_entries(item, entries);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_conversation_list_entries(item, entries);
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.starts_with('[') || trimmed.starts_with('{') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    collect_conversation_list_entries(&parsed, entries);
                }
            }
        }
        _ => {}
    }
}

fn parse_conversation_list_timestamp(value: &Value) -> Option<(i64, i64)> {
    let values = value.as_array()?;
    let secs = values
        .first()
        .and_then(Value::as_i64)
        .or_else(|| {
            values
                .first()
                .and_then(Value::as_u64)
                .and_then(|v| i64::try_from(v).ok())
        })
        .unwrap_or_default();
    let nanos = values
        .get(1)
        .and_then(Value::as_i64)
        .or_else(|| {
            values
                .get(1)
                .and_then(Value::as_u64)
                .and_then(|v| i64::try_from(v).ok())
        })
        .unwrap_or_default();
    Some((secs, nanos))
}

fn extract_response_id_from_conversation_list_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            looks_like_stream_generate_id(trimmed, "r_").then(|| trimmed.to_string())
        }
        Value::Array(items) => items
            .iter()
            .find_map(extract_response_id_from_conversation_list_value),
        Value::Object(map) => map
            .values()
            .find_map(extract_response_id_from_conversation_list_value),
        _ => None,
    }
}
