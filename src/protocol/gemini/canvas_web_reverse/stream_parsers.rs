use std::collections::HashMap;
use std::sync::OnceLock;

use base64::Engine;
use regex::Regex;
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas as legacy;

pub fn extract_audio_from_tts_export_response(
    body: &str,
) -> Result<legacy::GeminiCanvasAudio, GatewayError> {
    let normalized = strip_xssi_prefix(body);
    let (frames, _remainder) = parse_response_frames(normalized);
    let mut best_base64 = None;
    for frame in &frames {
        collect_tts_export_audio_base64(frame, &mut best_base64);
    }
    if best_base64.is_none() {
        best_base64 = scan_tts_export_audio_base64_in_text(normalized);
    }

    let best_base64 = match best_base64 {
        Some(payload) => payload,
        None if frames.is_empty() => {
            return Err(GatewayError::server_error(
                "Gemini Canvas TTS export response did not contain any parseable frames.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_tts_export_invalid_frame_response"));
        }
        None => {
            return Err(GatewayError::server_error(
                "Gemini Canvas TTS export response did not include an inline audio payload.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_tts_export_missing_audio"));
        }
    };
    decode_tts_export_audio_base64(&best_base64)
}

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

pub fn extract_video_generation_job_id(body: &str) -> Option<String> {
    let normalized = normalize_page_blob_media_text(strip_xssi_prefix(body));
    if !normalized.contains("video_gen_chip") {
        return None;
    }

    static VIDEO_JOB_ID_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = VIDEO_JOB_ID_REGEX.get_or_init(|| {
        Regex::new(r#"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"#)
            .expect("video job id regex must compile")
    });
    regex
        .find(&normalized)
        .map(|matched| matched.as_str().to_string())
}

pub fn response_indicates_video_generation_pending(body: &str) -> bool {
    let normalized = normalize_page_blob_media_text(strip_xssi_prefix(body));
    normalized.contains("video_gen_chip")
        || normalized.contains("正在生成视频")
        || normalized.to_ascii_lowercase().contains("generating video")
        || ((body.contains("\"18\":\"") || body.contains("\\\"18\\\":\\\""))
            && (body.contains("\"21\":[") || body.contains("\\\"21\\\":["))
            && (body.contains("\"44\":true") || body.contains("\\\"44\\\":true")))
        || normalized.contains("BardErrorInfo\",[1053]")
        || normalized.contains("BardErrorInfo\", [1053]")
}

pub fn response_indicates_video_generation_quota_reached(body: &str) -> bool {
    let normalized = normalize_page_blob_media_text(strip_xssi_prefix(body));
    let lower = normalized.to_ascii_lowercase();
    normalized.contains("已达到视频生成数量上限")
        || normalized.contains("后方可继续生成视频")
        || lower.contains("video generation quota is currently exhausted")
        || lower.contains("video generation limit has been reached")
}

fn parse_stream_frames_or_envelopes(normalized: &str) -> Vec<Value> {
    let (parsed_frames, _remainder) = parse_response_frames(normalized);
    if parsed_frames.is_empty() {
        parse_locator_response_envelopes(normalized)
    } else {
        parsed_frames
    }
}

fn strip_xssi_prefix(body: &str) -> &str {
    body.trim_start()
        .strip_prefix(")]}'")
        .map(str::trim_start)
        .unwrap_or(body)
}

fn looks_like_stream_generate_id(value: &str, prefix: &str) -> bool {
    let candidate = value.trim();
    candidate.starts_with(prefix)
        && candidate.len() > prefix.len()
        && candidate[prefix.len()..]
            .chars()
            .all(|ch| ch.is_ascii_hexdigit())
}

fn parse_locator_response_envelopes(content: &str) -> Vec<Value> {
    let content = content.trim();
    if content.is_empty() {
        return Vec::new();
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(content) {
        return match parsed {
            Value::Array(items) => items,
            other => vec![other],
        };
    }

    let mut collected = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(line) {
            match parsed {
                Value::Array(items) => collected.extend(items),
                other => collected.push(other),
            }
        }
    }

    collected
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

fn looks_like_tts_export_audio_base64(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 8 {
        return false;
    }
    (trimmed.starts_with("T2dnUw") || trimmed.starts_with("UklGR") || trimmed.starts_with("SUQz"))
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '\r' | '\n'))
}

fn infer_tts_export_audio_mime_type(base64_payload: &str) -> String {
    let trimmed = base64_payload.trim();
    if trimmed.starts_with("UklGR") {
        "audio/wav".to_string()
    } else if trimmed.starts_with("SUQz") {
        "audio/mpeg".to_string()
    } else {
        "audio/ogg".to_string()
    }
}

fn decode_tts_export_audio_base64(
    base64_payload: &str,
) -> Result<legacy::GeminiCanvasAudio, GatewayError> {
    let normalized = normalize_tts_export_audio_base64(base64_payload);
    let mime_type = infer_tts_export_audio_mime_type(&normalized);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(normalized.as_bytes())
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to decode Gemini Canvas TTS export audio payload: {error}"
            ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_tts_export_invalid_audio_base64")
        })?;
    Ok(legacy::GeminiCanvasAudio { mime_type, bytes })
}

fn normalize_tts_export_audio_base64(base64_payload: &str) -> String {
    let mut normalized = base64_payload
        .chars()
        .filter(|ch| !matches!(ch, '\r' | '\n' | ' ' | '\t'))
        .collect::<String>();
    let remainder = normalized.len() % 4;
    if remainder != 0 {
        normalized.extend(std::iter::repeat_n('=', 4 - remainder));
    }
    normalized
}

fn collect_tts_export_audio_base64(value: &Value, best_base64: &mut Option<String>) {
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if looks_like_tts_export_audio_base64(trimmed) {
            let should_replace = best_base64
                .as_ref()
                .map(|current| trimmed.len() > current.len())
                .unwrap_or(true);
            if should_replace {
                *best_base64 = Some(trimmed.to_string());
            }
        } else if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            collect_tts_export_audio_base64(&parsed, best_base64);
        }
    }
    if let Some(items) = value.as_array() {
        for item in items {
            collect_tts_export_audio_base64(item, best_base64);
        }
    }
    if let Some(object) = value.as_object() {
        for item in object.values() {
            collect_tts_export_audio_base64(item, best_base64);
        }
    }
}

fn scan_tts_export_audio_base64_in_text(body: &str) -> Option<String> {
    let search_body = body
        .split_once(legacy::GEMINI_CANVAS_TTS_EXPORT_RPCID)
        .map(|(_, suffix)| suffix)
        .unwrap_or(body);
    let mut best_base64 = None;
    for prefix in ["T2dnUw", "UklGR", "SUQz"] {
        let mut search_start = 0usize;
        while search_start < search_body.len() {
            let Some(relative_start) = search_body[search_start..].find(prefix) else {
                break;
            };
            let start = search_start + relative_start;
            let end = scan_tts_export_audio_base64_end(search_body, start);
            if end > start {
                let candidate = search_body[start..end].trim();
                let normalized_candidate = normalize_tts_export_audio_base64(candidate);
                if looks_like_tts_export_audio_base64(&normalized_candidate) {
                    let should_replace = best_base64
                        .as_ref()
                        .map(|current: &String| normalized_candidate.len() > current.len())
                        .unwrap_or(true);
                    if should_replace {
                        best_base64 = Some(normalized_candidate);
                    }
                }
            }
            search_start = start + prefix.len();
        }
    }
    best_base64
}

fn scan_tts_export_audio_base64_end(body: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in body[start..].char_indices() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '\r' | '\n') {
            end = start + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    end
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

fn parse_response_frames(content: &str) -> (Vec<Value>, String) {
    let mut consumed_chars = 0usize;
    let mut frames = Vec::new();
    while consumed_chars < content.len() {
        let mut index = consumed_chars;
        while let Some(ch) = content[index..].chars().next() {
            if ch.is_whitespace() {
                index += ch.len_utf8();
            } else {
                break;
            }
            if index >= content.len() {
                break;
            }
        }
        if index >= content.len() {
            consumed_chars = index;
            break;
        }

        let mut digit_end = index;
        while let Some(ch) = content[digit_end..].chars().next() {
            if ch.is_ascii_digit() {
                digit_end += ch.len_utf8();
            } else {
                break;
            }
            if digit_end >= content.len() {
                break;
            }
        }
        if digit_end == index {
            break;
        }
        let line_ending = if content[digit_end..].starts_with("\r\n") {
            2
        } else if content[digit_end..].starts_with('\n') {
            1
        } else {
            break;
        };
        let length = match content[index..digit_end].parse::<usize>() {
            Ok(value) => value,
            Err(_) => break,
        };
        let start_content = digit_end + line_ending;
        let Some((end_pos, parsed)) = parse_response_frame_chunk(content, start_content, length)
        else {
            break;
        };
        consumed_chars = end_pos;
        match parsed {
            Value::Array(items) => frames.extend(items),
            other => frames.push(other),
        }
    }

    (frames, content[consumed_chars..].to_string())
}

fn parse_response_frame_chunk(
    content: &str,
    start: usize,
    expected_length: usize,
) -> Option<(usize, Value)> {
    for end in [
        utf16_end_index(content, start, expected_length),
        fallback_line_json_end_index(content, start),
    ]
    .into_iter()
    .flatten()
    {
        let chunk = content[start..end].trim();
        if chunk.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(chunk) {
            return Some((end, parsed));
        }
    }

    None
}

fn fallback_line_json_end_index(content: &str, start: usize) -> Option<usize> {
    if start >= content.len() {
        return None;
    }
    let remainder = &content[start..];
    let line_end_offset = remainder.find('\n').unwrap_or(remainder.len());
    let mut end = start + line_end_offset;
    if end > start && content.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
        end -= 1;
    }
    let chunk = content[start..end].trim();
    if chunk.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(chunk).ok().map(|_| end)
}

fn utf16_end_index(content: &str, start: usize, units: usize) -> Option<usize> {
    let mut consumed_units = 0usize;
    for (offset, ch) in content[start..].char_indices() {
        let width = ch.len_utf16();
        if consumed_units + width > units {
            break;
        }
        consumed_units += width;
        if consumed_units == units {
            return Some(start + offset + ch.len_utf8());
        }
    }
    None
}

fn normalize_page_blob_media_text(body: &str) -> String {
    body.replace("\\u003d", "=")
        .replace("\\u0026", "&")
        .replace("\\u003a", ":")
        .replace("\\u002f", "/")
        .replace("\\u003f", "?")
        .replace("\\u0025", "%")
        .replace("\\/", "/")
        .replace("&amp;", "&")
}
