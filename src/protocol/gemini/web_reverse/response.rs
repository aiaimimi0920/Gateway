use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use serde_json::{json, Value};

use super::{GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE, GEMINI_WEB_SESSION_INVALID_CODE};
use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage};

pub fn classify_gemini_web_http_error(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    let provider = "gemini_web_compatible";
    if response_indicates_browser_challenge(status, content_type, body) {
        let mut error = GatewayError::service_unavailable(
            "Gemini Web direct replay hit an upstream browser challenge and requires fresh session material.",
        )
        .with_provider(provider)
        .with_code(GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE);
        error.http_status = Some(status);
        return error;
    }
    if response_indicates_session_invalid(status, content_type, body) {
        let mut error = GatewayError::unauthorized(
            "Gemini Web browser session is invalid or expired and must be refreshed before replay can continue.",
        )
        .with_provider(provider)
        .with_code(GEMINI_WEB_SESSION_INVALID_CODE);
        error.http_status = Some(status);
        return error;
    }
    classify_upstream_error(status, body, Some(provider))
}

pub fn response_indicates_browser_challenge(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let challenge_like = lower.contains("captcha")
        || lower.contains("challenge")
        || lower.contains("verify you are human")
        || lower.contains("sorry/index")
        || lower.contains("bot verification")
        || lower.contains("unusual traffic")
        || lower.contains("security check");
    let gemini_shell_like =
        lower.contains("bardchatui") || lower.contains("gemini.gstatic.com/_/mss/boq-bard-web");
    if status == 200 && html_like && gemini_shell_like {
        return false;
    }
    html_like && challenge_like && matches!(status, 200 | 403 | 429 | 503)
}

pub fn response_indicates_session_invalid(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let auth_like = lower.contains("accounts.google.com")
        || lower.contains("/signin")
        || lower.contains("sign in")
        || lower.contains("log in")
        || lower.contains("unauthorized")
        || lower.contains("session expired");
    let gemini_shell_like =
        lower.contains("bardchatui") || lower.contains("gemini.gstatic.com/_/mss/boq-bard-web");
    if status == 200 && html_like && auth_like && gemini_shell_like {
        return false;
    }
    matches!(status, 401 | 403 | 200) && html_like && auth_like
}

pub fn accumulate_gemini_web_response(
    body: &str,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let preview = |value: &str| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            trimmed.chars().take(220).collect::<String>()
        }
    };
    let tail_preview = |value: &str| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            let chars: Vec<char> = trimmed.chars().collect();
            let start = chars.len().saturating_sub(220);
            chars[start..].iter().collect::<String>()
        }
    };
    let normalized = strip_xssi_prefix(body);
    let frames = parse_response_envelopes(normalized);
    if frames.is_empty() {
        return Err(GatewayError::server_error(format!(
            "Gemini Web response did not contain any parseable frames. body_head={}; body_tail={}",
            preview(normalized),
            tail_preview(normalized)
        ))
        .with_provider("gemini_web_compatible")
        .with_code("gemini_web_invalid_frame_response"));
    }

    let mut best_result = None;
    for frame in &frames {
        if let Some(result) = extract_generate_response(frame, model) {
            best_result = select_more_complete_response(best_result, result);
        }
    }

    if let Some(result) = best_result {
        return Ok(result);
    }

    Err(GatewayError::server_error(format!(
        "Gemini Web response frames did not include a usable text candidate. frame_count={}; body_head={}; body_tail={}",
        frames.len(),
        preview(normalized),
        tail_preview(normalized)
    ))
    .with_provider("gemini_web_compatible")
    .with_code("gemini_web_missing_candidate_text"))
}

pub fn parse_response(body: &str, model: &str) -> Result<CanonicalRelayResponse, GatewayError> {
    accumulate_gemini_web_response(body, model)
}

pub fn translate_gemini_web_to_openai_sse(
    body: &str,
    model: &str,
) -> Result<Vec<Bytes>, GatewayError> {
    let canonical = accumulate_gemini_web_response(body, model)?;
    let response_id = format!("chatcmpl_geminiweb_{}", uuid::Uuid::new_v4().simple());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": canonical.model,
        "choices": [{
            "index": 0,
            "delta": { "content": canonical.text },
            "finish_reason": canonical.finish_reason,
        }]
    });
    Ok(vec![
        Bytes::from(format!("data: {chunk}\n\n")),
        Bytes::from_static(b"data: [DONE]\n\n"),
    ])
}

fn strip_xssi_prefix(body: &str) -> &str {
    body.trim_start()
        .strip_prefix(")]}'")
        .map(str::trim_start)
        .unwrap_or(body)
}

fn parse_response_envelopes(content: &str) -> Vec<Value> {
    let (frames, _remainder) = parse_response_frames(content);
    if !frames.is_empty() {
        return frames;
    }

    let content_stripped = content.trim();
    if content_stripped.is_empty() {
        return Vec::new();
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(content_stripped) {
        return match parsed {
            Value::Array(items) => items,
            other => vec![other],
        };
    }

    let mut collected = Vec::new();
    for line in content_stripped.lines() {
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
        let Some(end_pos) = utf16_end_index(content, start_content, length) else {
            break;
        };
        let chunk = content[start_content..end_pos].trim();
        consumed_chars = end_pos;
        if chunk.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(chunk) {
            match parsed {
                Value::Array(items) => frames.extend(items),
                other => frames.push(other),
            }
        }
    }

    (frames, content[consumed_chars..].to_string())
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

fn extract_generate_response(
    frame: &Value,
    fallback_model: &str,
) -> Option<CanonicalRelayResponse> {
    let mut best_result = None;

    if let Some(result) = extract_direct_generate_response(frame, fallback_model) {
        best_result = select_more_complete_response(best_result, result);
    }

    if let Some(inner_payload) = frame
        .as_array()
        .and_then(|items| items.get(2))
        .and_then(Value::as_str)
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
    {
        if let Some(result) = extract_direct_generate_response(&inner_payload, fallback_model) {
            best_result = select_more_complete_response(best_result, result);
        }
        if let Some(text) = get_nested_value(&inner_payload, &[4, 0, 1, 0]).and_then(Value::as_str)
        {
            let model = get_nested_value(&inner_payload, &[83]).and_then(Value::as_str);
            let result = CanonicalRelayResponse {
                model: model.unwrap_or(fallback_model).to_string(),
                text: text.to_string(),
                usage: None,
                tool_calls: Vec::new(),
                upstream_status: Some(200),
                finish_reason: Some("stop".to_string()),
            };
            best_result = select_more_complete_response(best_result, result);
        }
    }

    if let Some(items) = frame.as_array() {
        for item in items {
            if let Some(result) = extract_generate_response(item, fallback_model) {
                best_result = select_more_complete_response(best_result, result);
            }
        }
    }

    best_result
}

fn extract_direct_generate_response(
    value: &Value,
    fallback_model: &str,
) -> Option<CanonicalRelayResponse> {
    let mut tool_calls = Vec::new();
    if let Some(text) = value
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        return Some(CanonicalRelayResponse {
            model: value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(fallback_model)
                .to_string(),
            text,
            usage: extract_usage(value.get("usage")),
            tool_calls,
            upstream_status: Some(200),
            finish_reason: Some("stop".to_string()),
        });
    }

    let candidates = value.get("candidates").and_then(Value::as_array)?;
    let candidate = candidates.first()?;
    if let Some(parts) = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
    {
        for part in parts {
            if let Some(function_call) = part
                .get("functionCall")
                .or_else(|| part.get("function_call"))
            {
                let tool_call = parse_direct_generate_tool_call(function_call);
                if !tool_calls.iter().any(|existing: &CanonicalToolCall| {
                    existing.id == tool_call.id
                        && existing.name == tool_call.name
                        && existing.arguments == tool_call.arguments
                }) {
                    tool_calls.push(tool_call);
                }
            }
        }
    }

    let text = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .and_then(|parts| {
            let mut fragments = Vec::new();
            for part in parts {
                if let Some(value) = part.get("text").and_then(Value::as_str) {
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        fragments.push(trimmed.to_string());
                    }
                }
            }
            if fragments.is_empty() {
                None
            } else {
                Some(fragments.join(""))
            }
        })
        .or_else(|| {
            candidate
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            candidate
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        });

    if text.is_none() && tool_calls.is_empty() {
        return None;
    }

    let finish_reason = candidate
        .get("finishReason")
        .or_else(|| candidate.get("finish_reason"))
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .or_else(|| {
            Some(if tool_calls.is_empty() {
                "stop".to_string()
            } else {
                "tool_calls".to_string()
            })
        });

    Some(CanonicalRelayResponse {
        model: value
            .get("modelVersion")
            .or_else(|| value.get("model"))
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_string(),
        text: text.unwrap_or_default(),
        usage: extract_usage(value.get("usage")),
        tool_calls,
        upstream_status: Some(200),
        finish_reason,
    })
}

fn parse_direct_generate_tool_call(raw: &Value) -> CanonicalToolCall {
    CanonicalToolCall {
        id: raw
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        call_type: "function".to_string(),
        name: raw
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        arguments: raw
            .get("args")
            .or_else(|| raw.get("arguments"))
            .map(|value| {
                if let Some(text) = value.as_str() {
                    text.to_string()
                } else {
                    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
                }
            }),
        raw: HashMap::new(),
    }
}

fn extract_usage(value: Option<&Value>) -> Option<TokenUsage> {
    let usage = value?;
    Some(TokenUsage {
        prompt_tokens: usage
            .get("prompt_tokens")
            .or_else(|| usage.get("inputTokens"))
            .and_then(Value::as_u64)?,
        completion_tokens: usage
            .get("completion_tokens")
            .or_else(|| usage.get("outputTokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: usage
            .get("total_tokens")
            .or_else(|| usage.get("totalTokens"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                usage
                    .get("prompt_tokens")
                    .or_else(|| usage.get("inputTokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    + usage
                        .get("completion_tokens")
                        .or_else(|| usage.get("outputTokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
            }),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    })
}

fn get_nested_value<'a>(value: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut current = value;
    for index in path {
        current = current.as_array()?.get(*index)?;
    }
    Some(current)
}

fn select_more_complete_response(
    current: Option<CanonicalRelayResponse>,
    candidate: CanonicalRelayResponse,
) -> Option<CanonicalRelayResponse> {
    match current {
        None => Some(candidate),
        Some(existing) => {
            let existing_has_tool_calls = !existing.tool_calls.is_empty();
            let candidate_has_tool_calls = !candidate.tool_calls.is_empty();
            if candidate_has_tool_calls && !existing_has_tool_calls {
                return Some(candidate);
            }
            if existing_has_tool_calls && !candidate_has_tool_calls {
                return Some(existing);
            }
            let existing_len = existing.text.trim().chars().count();
            let candidate_len = candidate.text.trim().chars().count();
            if candidate_len > existing_len {
                Some(candidate)
            } else if candidate_len == existing_len
                && candidate.usage.is_some()
                && existing.usage.is_none()
            {
                Some(candidate)
            } else if candidate_len == existing_len {
                Some(candidate)
            } else {
                Some(existing)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed_body(payload: &Value) -> String {
        let frame_json = serde_json::to_string(payload).unwrap();
        let frame_len = frame_json.encode_utf16().count();
        format!("{frame_len}\n{frame_json}\n")
    }

    fn framed_body_crlf(payload: &Value) -> String {
        let frame_json = serde_json::to_string(payload).unwrap();
        let frame_len = frame_json.encode_utf16().count();
        format!("{frame_len}\r\n{frame_json}\r\n")
    }

    fn framed_item_body(payloads: &[Value]) -> String {
        payloads
            .iter()
            .map(|payload| framed_body(&json!([payload])))
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn session_invalid_ignores_gemini_shell_with_signin_promo() {
        let html = r#"
<!doctype html><html><head>
<script src="https://gemini.gstatic.com/_/mss/boq-bard-web/_/js/k=boq-bard-web.BardChatUi.en_US.test/am=1/d=1/rs=test"></script>
</head><body>
<a aria-label="Sign in" href="https://accounts.google.com/ServiceLogin?continue=https://gemini.google.com/app">Sign in</a>
</body></html>
"#;
        assert!(!response_indicates_session_invalid(
            200,
            Some("text/html; charset=utf-8"),
            html,
        ));
    }

    #[test]
    fn session_invalid_still_detects_plain_auth_gate_html() {
        let html = r#"
<!doctype html><html><body>
<a href="https://accounts.google.com/ServiceLogin">Sign in</a>
</body></html>
"#;
        assert!(response_indicates_session_invalid(
            200,
            Some("text/html; charset=utf-8"),
            html,
        ));
    }

    #[test]
    fn browser_challenge_ignores_gemini_shell_with_incidental_challenge_text() {
        let html = r#"
<!doctype html><html><head>
<script src="https://gemini.gstatic.com/_/mss/boq-bard-web/_/js/k=boq-bard-web.BardChatUi.en_US.test"></script>
<script>window.challengeConfiguration = {};</script>
</head><body>Gemini</body></html>
"#;
        assert!(!response_indicates_browser_challenge(
            200,
            Some("text/html; charset=utf-8"),
            html,
        ));
    }

    #[test]
    fn browser_challenge_still_detects_plain_captcha_html() {
        let html = r#"
<!doctype html><html><body>
<h1>Verify you are human</h1><div class="captcha">Security check</div>
</body></html>
"#;
        assert!(response_indicates_browser_challenge(
            200,
            Some("text/html; charset=utf-8"),
            html,
        ));
    }

    #[test]
    fn accumulates_direct_text_frame_response() {
        let body = framed_body(&json!({
            "text": "gemini web fixture ok",
            "model": "gemini-web"
        }));
        let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
        assert_eq!(canonical.model, "gemini-web");
        assert_eq!(canonical.text, "gemini web fixture ok");
    }

    #[test]
    fn accumulates_direct_text_frame_response_with_crlf() {
        let body = framed_body_crlf(&json!({
            "text": "gemini web fixture ok",
            "model": "gemini-web"
        }));
        let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
        assert_eq!(canonical.model, "gemini-web");
        assert_eq!(canonical.text, "gemini web fixture ok");
    }

    #[test]
    fn accumulates_plain_json_object_response_without_frames() {
        let body = serde_json::to_string(&json!({
            "text": "gemini web fixture ok",
            "model": "gemini-web"
        }))
        .unwrap();
        let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
        assert_eq!(canonical.model, "gemini-web");
        assert_eq!(canonical.text, "gemini web fixture ok");
    }

    #[test]
    fn accumulates_ndjson_response_without_frames() {
        let first = serde_json::to_string(&json!({
            "text": "gemini web fixture ok",
            "model": "gemini-web"
        }))
        .unwrap();
        let second = serde_json::to_string(&json!({
            "text": "ignored",
            "model": "gemini-web-2"
        }))
        .unwrap();
        let body = format!("{first}\n{second}\n");
        let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
        assert_eq!(canonical.model, "gemini-web");
        assert_eq!(canonical.text, "gemini web fixture ok");
    }

    #[test]
    fn accumulates_latest_incremental_canvas_frame_response() {
        let first_payload = json!([
            null,
            ["conversation-1", "response-1"],
            null,
            null,
            [["candidate-1", ["openai chat"]]]
        ]);
        let second_payload = json!([
            null,
            ["conversation-1", "response-1"],
            null,
            null,
            [["candidate-1", ["openai chat basic nonstream ok"]]],
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            "gemini-3-flash-preview"
        ]);
        let body = framed_item_body(&[
            json!([
                "wrb.fr",
                null,
                serde_json::to_string(&first_payload).unwrap()
            ]),
            json!([
                "wrb.fr",
                null,
                serde_json::to_string(&second_payload).unwrap()
            ]),
        ]);
        let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
        assert_eq!(canonical.text, "openai chat basic nonstream ok");
    }

    #[test]
    fn extracts_nested_candidate_text_from_inner_payload() {
        let inner = json!([null, null, null, null, [[null, ["gemini nested ok"]]]]);
        let frame = json!([null, null, serde_json::to_string(&inner).unwrap()]);
        let canonical = extract_generate_response(&frame, "fallback").unwrap();
        assert_eq!(canonical.text, "gemini nested ok");
    }

    #[test]
    fn accumulates_direct_tool_call_frame_response_without_text() {
        let body = framed_body(&json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "weather",
                            "args": { "city": "Hangzhou" }
                        }
                    }]
                }
            }],
            "modelVersion": "gemini-web-tool-only"
        }));
        let canonical = accumulate_gemini_web_response(&body, "fallback").unwrap();
        assert_eq!(canonical.model, "gemini-web-tool-only");
        assert_eq!(canonical.text, "");
        assert_eq!(canonical.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(canonical.tool_calls.len(), 1);
        assert_eq!(canonical.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            canonical.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    #[test]
    fn extracts_nested_tool_call_only_from_inner_payload() {
        let inner = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "weather",
                            "args": { "city": "Hangzhou" }
                        }
                    }]
                }
            }],
            "modelVersion": "gemini-web-inner-tool-only"
        });
        let frame = json!([null, null, serde_json::to_string(&inner).unwrap()]);
        let canonical = extract_generate_response(&frame, "fallback").unwrap();
        assert_eq!(canonical.model, "gemini-web-inner-tool-only");
        assert_eq!(canonical.text, "");
        assert_eq!(canonical.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(canonical.tool_calls.len(), 1);
        assert_eq!(canonical.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            canonical.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    #[test]
    fn translates_nonstream_response_to_openai_chunks() {
        let body = framed_body(&json!({
            "text": "gemini web fixture ok",
            "model": "gemini-web"
        }));
        let chunks = translate_gemini_web_to_openai_sse(&body, "fallback").unwrap();
        assert_eq!(chunks.len(), 2);
        let first = std::str::from_utf8(chunks[0].as_ref()).unwrap();
        assert!(first.contains("gemini web fixture ok"));
        let second = std::str::from_utf8(chunks[1].as_ref()).unwrap();
        assert_eq!(second, "data: [DONE]\n\n");
    }
}
