use serde_json::Value;

use super::{GeminiBusinessCaptureOutput, GeminiCanvasCaptureOutput, GeminiWebCaptureOutput};

pub(super) fn parse_remote_gemini_canvas_capture_output(
    value: Value,
) -> Result<GeminiCanvasCaptureOutput, String> {
    let parsed = serde_json::from_value::<GeminiCanvasCaptureOutput>(value).map_err(|error| {
        format!("Gemini host browser executor returned an invalid Gemini Canvas payload: {error}")
    })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini host browser executor failed.".to_string()));
    }
    let runtime_state_object_key = parsed
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| {
            "Gemini host browser executor did not return runtimeStateObjectKey.".to_string()
        })?;
    let _ = runtime_state_object_key;
    Ok(parsed)
}

pub(super) fn parse_remote_gemini_business_capture_output(
    value: Value,
) -> Result<GeminiBusinessCaptureOutput, String> {
    let parsed = serde_json::from_value::<GeminiBusinessCaptureOutput>(value).map_err(|error| {
        format!("Gemini host browser executor returned an invalid Gemini Business payload: {error}")
    })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini host browser executor failed.".to_string()));
    }
    let jwt = parsed
        .jwt
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "Gemini host browser executor did not return jwt.".to_string())?;
    let config_id = parsed
        .config_id
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "Gemini host browser executor did not return configId.".to_string())?;
    let session = parsed
        .session
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| "Gemini host browser executor did not return session.".to_string())?;
    let _ = (jwt, config_id, session);
    Ok(parsed)
}

pub(super) fn parse_remote_gemini_web_capture_output(
    value: Value,
) -> Result<GeminiWebCaptureOutput, String> {
    let parsed = serde_json::from_value::<GeminiWebCaptureOutput>(value).map_err(|error| {
        format!("Gemini host browser executor returned an invalid Gemini Web payload: {error}")
    })?;
    validate_gemini_web_capture_output(parsed, "Gemini host browser executor")
}

pub(super) fn parse_gemini_canvas_capture_output(
    stdout: &str,
    stderr: &str,
    exited_successfully: bool,
) -> Result<GeminiCanvasCaptureOutput, String> {
    let combined = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    };
    let json_text = extract_last_json_object(&combined).ok_or_else(|| {
        format!(
            "Gemini Canvas auth helper returned no parseable JSON output. stdout: {} stderr: {}",
            summarize_output(stdout),
            summarize_output(stderr)
        )
    })?;
    let parsed =
        serde_json::from_str::<GeminiCanvasCaptureOutput>(&json_text).map_err(|error| {
            format!(
                "Failed to parse Gemini Canvas auth helper JSON output: {error}. payload: {}",
                summarize_output(&json_text)
            )
        })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini Canvas auth helper failed.".to_string()));
    }
    let runtime_state_object_key = parsed
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Gemini Canvas auth helper did not return runtimeStateObjectKey.".to_string()
        })?;
    if !exited_successfully {
        return Err(format!(
            "Gemini Canvas auth helper exited before completing successfully but returned runtimeStateObjectKey={runtime_state_object_key}."
        ));
    }
    Ok(parsed)
}

pub(super) fn parse_gemini_business_capture_output(
    stdout: &str,
    stderr: &str,
    exited_successfully: bool,
) -> Result<GeminiBusinessCaptureOutput, String> {
    let combined = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    };
    let json_text = extract_last_json_object(&combined).ok_or_else(|| {
        format!(
            "Gemini Business auth helper returned no parseable JSON output. stdout: {} stderr: {}",
            summarize_output(stdout),
            summarize_output(stderr)
        )
    })?;
    let parsed =
        serde_json::from_str::<GeminiBusinessCaptureOutput>(&json_text).map_err(|error| {
            format!(
                "Failed to parse Gemini Business auth helper JSON output: {error}. payload: {}",
                summarize_output(&json_text)
            )
        })?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Gemini Business auth helper failed.".to_string()));
    }
    let jwt = parsed
        .jwt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Gemini Business auth helper did not return jwt.".to_string())?;
    let config_id = parsed
        .config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Gemini Business auth helper did not return configId.".to_string())?;
    let session = parsed
        .session
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Gemini Business auth helper did not return session.".to_string())?;
    if !exited_successfully {
        return Err(format!(
            "Gemini Business auth helper exited before completing successfully but returned jwt/configId/session. jwt_len={} configId={} session={}",
            jwt.len(),
            config_id,
            session
        ));
    }
    Ok(parsed)
}

pub(super) fn parse_gemini_web_capture_output(
    stdout: &str,
    stderr: &str,
    exited_successfully: bool,
) -> Result<GeminiWebCaptureOutput, String> {
    let combined = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    };
    let json_text = extract_last_json_object(&combined).ok_or_else(|| {
        format!(
            "Gemini Web auth helper returned no parseable JSON output. stdout: {} stderr: {}",
            summarize_output(stdout),
            summarize_output(stderr)
        )
    })?;
    let parsed = serde_json::from_str::<GeminiWebCaptureOutput>(&json_text).map_err(|error| {
        format!(
            "Failed to parse Gemini Web auth helper JSON output: {error}. payload: {}",
            summarize_output(&json_text)
        )
    })?;
    let parsed = validate_gemini_web_capture_output(parsed, "Gemini Web auth helper")?;
    if !exited_successfully {
        return Err(
            "Gemini Web auth helper exited before completing successfully after returning runtime material."
                .to_string(),
        );
    }
    Ok(parsed)
}

fn validate_gemini_web_capture_output(
    parsed: GeminiWebCaptureOutput,
    source: &str,
) -> Result<GeminiWebCaptureOutput, String> {
    if !parsed.ok {
        return Err(parsed
            .error
            .clone()
            .unwrap_or_else(|| format!("{source} failed.")));
    }
    parsed
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{source} did not return the primary Gemini Web cookie."))?;
    let response_status = parsed
        .response_status
        .ok_or_else(|| format!("{source} did not return the validation response status."))?;
    if !(200..300).contains(&response_status) {
        return Err(format!(
            "{source} captured the validation request, but Gemini Web returned HTTP {response_status}."
        ));
    }
    if parsed.response_contains_paris != Some(true) {
        return Err(format!(
            "{source} captured the validation request, but the response did not contain the expected answer."
        ));
    }
    Ok(parsed)
}

pub(super) fn extract_last_json_object(text: &str) -> Option<String> {
    let starts = text
        .match_indices('{')
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    for start in starts.into_iter().rev() {
        let candidate = text[start..].trim();
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}

pub(super) fn summarize_output(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 400 {
        return trimmed.to_string();
    }
    format!("{}...(truncated)", &trimmed[..400])
}
