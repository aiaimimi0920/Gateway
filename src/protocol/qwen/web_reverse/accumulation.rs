use super::diagnostics::qwen_web_response_shape;
use super::http_errors::{classify_qwen_web_http_error, response_indicates_browser_challenge};
use super::response_value::apply_qwen_web_response_value;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, TokenUsage};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};
use crate::protocol::upstream_body::collect_bounded_upstream_text;
use serde_json::Value;

pub async fn accumulate_qwen_web_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = collect_bounded_upstream_text(response, "Qwen Web response body").await?;
    if response_indicates_browser_challenge(status, content_type.as_deref(), &body) {
        return Err(classify_qwen_web_http_error(
            status,
            content_type.as_deref(),
            &body,
        ));
    }
    accumulate_qwen_web_body(&body, model)
}

pub(super) fn accumulate_qwen_web_body(
    body: &str,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut parser = SseParseState::new();
    let mut text = String::new();
    let mut reported_model = model.to_string();
    let mut usage: Option<TokenUsage> = None;
    let mut finish_reason: Option<String> = None;
    let mut parsed_frame = false;

    for raw_line in body.lines() {
        let normalized_line = raw_line.trim_end_matches('\r');
        if let Some(data_line) = normalized_line.strip_prefix("data:") {
            let data_line = data_line.strip_prefix(' ').unwrap_or(data_line).trim();
            if data_line == "[DONE]" {
                break;
            }
            if let Ok(data) = serde_json::from_str::<Value>(data_line) {
                parsed_frame |= apply_qwen_web_response_value(
                    Some(&data),
                    &mut reported_model,
                    &mut text,
                    &mut usage,
                    &mut finish_reason,
                );
                continue;
            }
        }
        if let Some(frame) = parse_sse_line(raw_line, &mut parser) {
            if frame.data == "[DONE]" {
                break;
            }
            parsed_frame |= apply_qwen_web_response_value(
                serde_json::from_str(&frame.data).ok().as_ref(),
                &mut reported_model,
                &mut text,
                &mut usage,
                &mut finish_reason,
            );
        }
    }

    // A few Qwen deployments omit the final blank SSE delimiter. Flush the
    // parser explicitly so the last data frame is not lost at EOF.
    if let Some(frame) = parse_sse_line("", &mut parser) {
        if frame.data != "[DONE]" {
            parsed_frame |= apply_qwen_web_response_value(
                serde_json::from_str(&frame.data).ok().as_ref(),
                &mut reported_model,
                &mut text,
                &mut usage,
                &mut finish_reason,
            );
        }
    }

    // Some Qwen responses ignore the requested stream flag and return a
    // regular OpenAI-shaped JSON response instead of SSE.
    if !parsed_frame {
        parsed_frame |= apply_qwen_web_response_value(
            serde_json::from_str(body.trim()).ok().as_ref(),
            &mut reported_model,
            &mut text,
            &mut usage,
            &mut finish_reason,
        );
    }

    if !parsed_frame || text.trim().is_empty() {
        return Err(GatewayError::server_error(format!(
            "Qwen Web response did not include assistant text ({})",
            qwen_web_response_shape(body)
        ))
        .with_code("qwen_web_empty_response"));
    }

    Ok(CanonicalRelayResponse {
        model: reported_model,
        text,
        usage,
        tool_calls: Vec::new(),
        upstream_status: Some(200),
        finish_reason,
    })
}
