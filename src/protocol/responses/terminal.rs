//! A response-level terminal event, not a text/tool item or HTTP EOF, proves completion.
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::sse_parse::SseFrame;
use crate::protocol::stream_error::ProtocolStreamError;

#[derive(Clone, Debug)]
pub(super) enum CompletionError {
    Incomplete,
    InvalidEvent,
    Upstream(&'static str),
}

impl CompletionError {
    fn code(&self) -> &'static str {
        match self {
            Self::Incomplete => "responses_stream_incomplete",
            Self::InvalidEvent => "responses_stream_parse_failed",
            Self::Upstream(_) => "responses_stream_upstream_error",
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::Incomplete => {
                "Responses stream ended before response.completed or response.incomplete."
            }
            Self::InvalidEvent => "Responses stream contained an invalid or conflicting event.",
            Self::Upstream(message) => message,
        }
    }

    pub(super) fn protocol(self) -> ProtocolStreamError {
        // Only fixed local text leaves this owner, never provider prose, code or partial output.
        ProtocolStreamError::invalid_data(format!("{}: {}", self.code(), self.message()))
    }

    pub(super) fn gateway(self) -> GatewayError {
        GatewayError::server_error(self.message()).with_code(self.code())
    }
}

#[derive(Default)]
pub(super) struct Completion {
    completed: bool,
}

impl Completion {
    pub(super) fn observe(&mut self, frame: &SseFrame) -> Result<Option<Value>, CompletionError> {
        if frame.data.is_empty() {
            return Ok(None);
        }
        if frame.data == "[DONE]" {
            self.finish()?;
            return Ok(None);
        }
        let payload: Value =
            serde_json::from_str(&frame.data).map_err(|_| CompletionError::InvalidEvent)?;
        let payload_type = payload.get("type").and_then(Value::as_str);
        let event_type = frame
            .event_name
            .as_deref()
            .or(payload_type)
            .unwrap_or_default();
        if frame.event_name.is_some() && payload_type.is_some_and(|kind| kind != event_type) {
            return Err(CompletionError::InvalidEvent);
        }
        match event_type {
            "error" | "response.failed" | "response.cancelled" => {
                return Err(upstream_failure(&payload))
            }
            "response.completed" | "response.incomplete" => {
                let response = payload
                    .get("response")
                    .filter(|value| value.is_object())
                    .ok_or(CompletionError::InvalidEvent)?;
                let expected = if event_type == "response.completed" {
                    "completed"
                } else {
                    "incomplete"
                };
                // The existing reader accepts Chat's tool_calls status. Preserve only
                // that legacy wire shape with an actual completed function-call output.
                let legacy_tools = event_type == "response.completed"
                    && response.get("status").and_then(Value::as_str) == Some("tool_calls")
                    && response
                        .get("output")
                        .and_then(Value::as_array)
                        .is_some_and(|items| {
                            items.iter().any(|item| {
                                matches!(
                                    item.get("type").and_then(Value::as_str),
                                    Some("function_call" | "custom_tool_call")
                                )
                            })
                        });
                if response
                    .get("status")
                    .is_some_and(|value| value.as_str() != Some(expected))
                    && !legacy_tools
                {
                    return Err(CompletionError::InvalidEvent);
                }
                self.completed = true;
            }
            _ if payload.get("error").is_some_and(|error| !error.is_null()) => {
                return Err(upstream_failure(&payload));
            }
            _ => {}
        }
        Ok(Some(payload))
    }

    pub(super) fn finish(&self) -> Result<(), CompletionError> {
        if self.completed {
            Ok(())
        } else {
            Err(CompletionError::Incomplete)
        }
    }
}

fn upstream_failure(payload: &Value) -> CompletionError {
    let error = payload
        .get("error")
        .filter(|value| !value.is_null())
        .or_else(|| payload.pointer("/response/error"));
    let code = error
        .and_then(|e| e.get("code").or_else(|| e.get("type")))
        .and_then(Value::as_str);
    // Exact structured-code compatibility only. Arbitrary prose must not grant a health category;
    // these labels preserve existing classifiers without changing GatewayError/replay policy.
    CompletionError::Upstream(match code {
        Some("invalid_api_key" | "invalid_token" | "authentication_error" | "unauthorized") => {
            "Responses upstream stream failed (credential_invalid)."
        }
        Some("token_expired" | "expired_token" | "session_expired") => {
            "Responses upstream stream failed (token_expired)."
        }
        Some("model_not_found" | "model_not_supported" | "unsupported_model") => {
            "Responses upstream stream failed (model_unsupported)."
        }
        Some(
            "insufficient_quota"
            | "quota_exceeded"
            | "quota_exhausted"
            | "billing_hard_limit_reached",
        ) => "Responses upstream stream failed (quota_exhausted).",
        Some("rate_limit_exceeded" | "rate_limit_error" | "rate_limited" | "too_many_requests") => {
            "Responses upstream stream failed (rate_limited)."
        }
        Some(
            "content_filter"
            | "content_policy_violation"
            | "safety_violation"
            | "content_filter_error",
        ) => "Responses upstream stream failed (content_filter).",
        _ => "Responses upstream stream failed before completion.",
    })
}
