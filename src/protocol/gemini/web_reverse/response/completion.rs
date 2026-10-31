//! Media inputs require an observed completed candidate, not synthesized legacy stop.
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayResponse;

fn complete_text(value: &Value) -> Option<String> {
    if let Some(candidate) = value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
    {
        if candidate
            .get("finishReason")
            .or_else(|| candidate.get("finish_reason"))
            .and_then(Value::as_str)
            != Some("STOP")
        {
            return None;
        }
        let mut text = String::new();
        for part in candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)?
        {
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if part.get("functionCall").is_some() || part.get("function_call").is_some() {
                return None;
            }
            if let Some(fragment) = part.get("text").and_then(Value::as_str) {
                if text.len().saturating_add(fragment.len()) > 1024 * 1024 {
                    return None;
                }
                text.push_str(fragment);
            }
        }
        return (!text.trim().is_empty()).then_some(text);
    }
    // The Web candidate's observed completion indicator is [8][0] == 2.
    let candidate = value.pointer("/4/0")?;
    if candidate.pointer("/8/0").and_then(Value::as_u64) != Some(2) {
        return None;
    }
    let text = candidate.pointer("/1/0").and_then(Value::as_str)?;
    (!text.trim().is_empty() && text.len() <= 1024 * 1024).then(|| text.to_string())
}

fn inspect_frame(value: &Value) -> Option<String> {
    if let Some(text) = complete_text(value) {
        return Some(text);
    }
    if value.get(0).and_then(Value::as_str) == Some("wrb.fr") {
        let inner = value.get(2).and_then(Value::as_str)?;
        return complete_text(&serde_json::from_str::<Value>(inner).ok()?);
    }
    value.as_array()?.iter().find_map(inspect_frame)
}

pub(crate) fn accumulate_complete_media_response(
    body: &str,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut text = None;
    for frame in super::frames::parse_response_envelopes(super::frames::strip_xssi_prefix(body)) {
        if let Some(completed) = inspect_frame(&frame) {
            text = Some(completed);
        }
    }
    let text = text.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Web media response did not include a complete nonempty text candidate.",
        )
        .with_provider("gemini_web_compatible")
        .with_code("gemini_web_media_incomplete")
    })?;
    Ok(CanonicalRelayResponse {
        model_declaration: None,
        model: model.into(),
        text,
        usage: None,
        tool_calls: vec![],
        upstream_status: Some(200),
        finish_reason: Some("stop".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn partial_or_thought_only_is_not_a_transcript() {
        for body in [
            json!({"text":"partial"}),
            json!({"candidates":[{"content":{"parts":[{"text":"partial"}]}}]}),
            json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"thought":true,"text":"private thought"}]}}]}),
        ] {
            assert!(accumulate_complete_media_response(&body.to_string(), "fixture").is_err());
        }
    }

    #[test]
    fn real_web_completion_indicator_and_direct_stop_are_required() {
        for indicator in [1, 2] {
            let mut candidate = vec![Value::Null; 9];
            candidate[1] = json!(["合成回执"]);
            candidate[8] = json!([indicator]);
            let body = json!([[
                "wrb.fr",
                null,
                json!([null, null, null, null, [candidate]]).to_string()
            ]])
            .to_string();
            let result = accumulate_complete_media_response(&body, "fixture");
            assert_eq!(result.is_ok(), indicator == 2);
        }
        let body = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"thought":true,"text":"ignored"},{"text":"answer"}]}}]}).to_string();
        assert_eq!(
            accumulate_complete_media_response(&body, "fixture")
                .unwrap()
                .text,
            "answer"
        );
    }
}
