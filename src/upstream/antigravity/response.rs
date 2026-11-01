//! Cloud Code JSON/SSE completion evidence. Never let the generic parser invent success.
use super::failure;
use crate::error::GatewayError;
use serde_json::{json, Value};

fn unwrap(value: &Value) -> Result<Value, GatewayError> {
    let response = value
        .get("response")
        .filter(|v| v.is_object())
        .ok_or_else(|| {
            failure(
                "antigravity_envelope_invalid",
                "Antigravity response envelope is missing",
            )
        })?;
    if value.get("error").is_some_and(|v| !v.is_null())
        || response.get("error").is_some_and(|v| !v.is_null())
    {
        return Err(failure(
            "antigravity_generation_failed",
            "Antigravity generation returned an error; private body omitted",
        ));
    }
    if response
        .get("promptFeedback")
        .and_then(|v| v.get("blockReason"))
        .is_some_and(|v| !v.is_null())
    {
        return Err(failure(
            "antigravity_generation_blocked",
            "Antigravity blocked this generation",
        ));
    }
    Ok(response.clone())
}

fn terminal(response: &Value) -> Result<(), GatewayError> {
    let candidates = response["candidates"]
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or_else(|| {
            failure(
                "antigravity_candidate_invalid",
                "Antigravity requires one non-empty terminal candidate",
            )
        })?;
    match candidates[0]["finishReason"].as_str() {
        Some("STOP" | "MAX_TOKENS") => {}
        Some(
            "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" | "IMAGE_SAFETY",
        ) => {
            return Err(failure(
                "antigravity_generation_blocked",
                "Antigravity blocked this generation",
            ))
        }
        _ => {
            return Err(failure(
                "antigravity_response_incomplete",
                "Antigravity did not provide a supported terminal reason; not replayed",
            ))
        }
    }
    if !candidates[0]["content"]["parts"]
        .as_array()
        .is_some_and(|parts| {
            parts.iter().any(|part| {
                part["text"].as_str().is_some_and(|s| !s.is_empty())
                    || part["functionCall"].is_object()
                    || part["function_call"].is_object()
            })
        })
    {
        return Err(failure(
            "antigravity_response_empty",
            "Antigravity returned no text or tool output",
        ));
    }
    Ok(())
}

pub(super) fn decode(bytes: &[u8], stream: bool) -> Result<Value, GatewayError> {
    if !stream {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| failure("antigravity_invalid_json", "Invalid Antigravity JSON"))?;
        let response = unwrap(&value)?;
        terminal(&response)?;
        return Ok(response);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| {
            failure(
                "antigravity_invalid_sse",
                "Invalid Antigravity SSE encoding",
            )
        })?
        .replace("\r\n", "\n");
    // SSE dispatches only blank-line-terminated events. Never consume an EOF residual.
    let (complete, _residual) = text.rsplit_once("\n\n").ok_or_else(|| {
        failure(
            "antigravity_stream_incomplete",
            "Antigravity stream has no complete SSE event",
        )
    })?;
    let mut merged = json!({"candidates":[{"content":{"role":"model","parts":[]}}]});
    let mut finished = false;
    for event in complete.split("\n\n").filter(|s| !s.trim().is_empty()) {
        let data = event
            .lines()
            .filter_map(|s| s.strip_prefix("data:").map(str::trim_start))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: Value = serde_json::from_str(&data)
            .map_err(|_| failure("antigravity_invalid_sse", "Invalid Antigravity SSE data"))?;
        let response = unwrap(&value)?;
        if let Some(candidates) = response["candidates"].as_array() {
            if candidates.len() > 1 {
                return Err(failure(
                    "antigravity_candidate_invalid",
                    "Multiple Antigravity candidates are not supported",
                ));
            }
            for candidate in candidates {
                if let Some(parts) = candidate["content"]["parts"].as_array() {
                    if finished && !parts.is_empty() {
                        return Err(failure(
                            "antigravity_stream_after_terminal",
                            "Antigravity output continued after a terminal candidate",
                        ));
                    }
                    merged["candidates"][0]["content"]["parts"]
                        .as_array_mut()
                        .unwrap()
                        .extend(parts.iter().cloned());
                }
                if let Some(reason) = candidate.get("finishReason") {
                    if finished {
                        return Err(failure(
                            "antigravity_stream_duplicate_terminal",
                            "Antigravity returned conflicting terminal candidates",
                        ));
                    }
                    merged["candidates"][0]["finishReason"] = reason.clone();
                    // Validate immediately; a later STOP must not hide an invalid or blocked terminal.
                    terminal(&merged)?;
                    finished = true;
                }
            }
        }
        for key in ["usageMetadata", "modelVersion"] {
            if let Some(value) = response.get(key) {
                merged[key] = value.clone();
            }
        }
    }
    terminal(&merged)?;
    Ok(merged)
}
