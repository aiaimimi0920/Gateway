//! One bounded payload decoder; CLI Key/OAuth runtime reject ambiguous envelopes and known fields.
use super::event_stream::{EventStreamFrame, HeaderValue, KiroEvent};
use crate::error::GatewayError;
use serde_json::{json, Value};

pub(super) fn parse(frame: EventStreamFrame<'_>, strict: bool) -> Result<KiroEvent, GatewayError> {
    let message_type = frame
        .headers
        .get(":message-type")
        .and_then(HeaderValue::as_str)
        .unwrap_or("event");
    if strict
        && (frame
            .headers
            .get(":message-type")
            .and_then(HeaderValue::as_str)
            .is_none()
            || !matches!(message_type, "event" | "error" | "exception"))
    {
        return Err(super::key_response::invalid());
    }
    Ok(match message_type {
        "event" => payload(frame, strict)?,
        "error" => KiroEvent::Error {
            error_code: frame
                .headers
                .get(":error-code")
                .and_then(HeaderValue::as_str)
                .unwrap_or("UnknownError")
                .to_string(),
            error_message: String::from_utf8_lossy(frame.payload).to_string(),
        },
        "exception" => KiroEvent::Exception {
            exception_type: frame
                .headers
                .get(":exception-type")
                .and_then(HeaderValue::as_str)
                .unwrap_or("UnknownException")
                .to_string(),
            message: String::from_utf8_lossy(frame.payload).to_string(),
        },
        _ => KiroEvent::Unknown,
    })
}

fn payload(frame: EventStreamFrame<'_>, strict: bool) -> Result<KiroEvent, GatewayError> {
    let event_type = frame
        .headers
        .get(":event-type")
        .and_then(HeaderValue::as_str)
        .unwrap_or("");
    if strict && event_type.is_empty() {
        return Err(super::key_response::invalid());
    }
    if !matches!(
        event_type,
        "assistantResponseEvent" | "toolUseEvent" | "contextUsageEvent"
    ) {
        return Ok(KiroEvent::Unknown);
    }
    if strict {
        return strict_payload(event_type, frame.payload);
    }
    let value: Value = match serde_json::from_slice(frame.payload) {
        Ok(value) => value,
        Err(_) => json!({}),
    };
    Ok(match event_type {
        "assistantResponseEvent" => KiroEvent::AssistantResponse {
            content: value
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        },
        "toolUseEvent" => KiroEvent::ToolUse {
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            tool_use_id: value
                .get("toolUseId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            input: value
                .get("input")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            stop: value.get("stop").and_then(Value::as_bool).unwrap_or(false),
        },
        "contextUsageEvent" => KiroEvent::ContextUsage {
            context_usage_percentage: value
                .get("contextUsagePercentage")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        },
        _ => KiroEvent::Unknown,
    })
}

// Derived struct visitors reject repeated known members (including escaped aliases), before a
// serde_json::Value could overwrite them. Missing continuation fields default; explicit null fails.
fn strict_payload(event_type: &str, bytes: &[u8]) -> Result<KiroEvent, GatewayError> {
    // Serde's derived struct visitor also accepts positional arrays. Require an object without
    // first materializing Value (which would lose duplicate fields); serde validates full syntax.
    if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
        return Err(super::key_response::invalid());
    }
    let invalid = |_| super::key_response::invalid();
    Ok(match event_type {
        "assistantResponseEvent" => {
            let value: Assistant = serde_json::from_slice(bytes).map_err(invalid)?;
            KiroEvent::AssistantResponse {
                content: value.content,
            }
        }
        "toolUseEvent" => {
            let value: Tool = serde_json::from_slice(bytes).map_err(invalid)?;
            KiroEvent::ToolUse {
                name: value.name,
                tool_use_id: value.tool_use_id,
                input: value.input,
                stop: value.stop,
            }
        }
        "contextUsageEvent" => {
            let value: Context = serde_json::from_slice(bytes).map_err(invalid)?;
            if !value.percentage.is_finite() || value.percentage < 0.0 {
                return Err(super::key_response::invalid());
            }
            KiroEvent::ContextUsage {
                context_usage_percentage: value.percentage,
            }
        }
        _ => unreachable!("only known event types reach strict payload decoding"),
    })
}

#[derive(serde::Deserialize)]
struct Assistant {
    content: String,
}
#[derive(serde::Deserialize)]
struct Tool {
    #[serde(rename = "toolUseId")]
    tool_use_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    input: String,
    #[serde(default)]
    stop: bool,
}
#[derive(serde::Deserialize)]
struct Context {
    #[serde(rename = "contextUsagePercentage")]
    percentage: f64,
}
