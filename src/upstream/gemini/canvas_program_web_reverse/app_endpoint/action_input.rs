//! Extract operation-matched prompts and media hints from captured action input.

use crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig;
use crate::protocol::gemini_canvas;

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiCanvasProgramActionHints {
    pub prompt: Option<String>,
    pub duration_seconds: Option<f64>,
    pub aspect_ratio: Option<String>,
}

fn extract_prompt_from_action_input(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.starts_with('{') {
        return Some(trimmed.to_string());
    }
    for marker in ["'prompt'", "\"prompt\""] {
        let Some(marker_index) = raw.find(marker) else {
            continue;
        };
        let after_marker = &raw[marker_index + marker.len()..];
        let Some(colon_index) = after_marker.find(':') else {
            continue;
        };
        let after_colon = after_marker[colon_index + 1..].trim_start();
        let Some(quote) = after_colon
            .chars()
            .next()
            .filter(|ch| *ch == '\'' || *ch == '"')
        else {
            continue;
        };
        let remaining = &after_colon[quote.len_utf8()..];
        if let Some(end_index) = remaining.find(quote) {
            let prompt = remaining[..end_index].trim();
            if !prompt.is_empty() {
                return Some(prompt.to_string());
            }
        }
    }
    None
}

fn extract_raw_action_scalar<'a>(raw: &'a str, markers: &[&str]) -> Option<&'a str> {
    for marker in markers {
        let Some(marker_index) = raw.find(marker) else {
            continue;
        };
        let after_marker = &raw[marker_index + marker.len()..];
        let Some(colon_index) = after_marker.find(':') else {
            continue;
        };
        let after_colon = after_marker[colon_index + 1..].trim_start();
        if after_colon.is_empty() {
            continue;
        }
        if let Some(quote) = after_colon
            .chars()
            .next()
            .filter(|ch| *ch == '\'' || *ch == '"')
        {
            let remaining = &after_colon[quote.len_utf8()..];
            if let Some(end_index) = remaining.find(quote) {
                return Some(remaining[..end_index].trim());
            }
            continue;
        }
        let end_index = after_colon
            .find([',', '}', '\n'])
            .unwrap_or(after_colon.len());
        return Some(after_colon[..end_index].trim());
    }
    None
}

fn extract_duration_seconds_from_action_input(raw: &str) -> Option<f64> {
    extract_raw_action_scalar(
        raw,
        &[
            "'duration_seconds'",
            "\"duration_seconds\"",
            "'durationSeconds'",
            "\"durationSeconds\"",
            "'duration'",
            "\"duration\"",
        ],
    )
    .and_then(|value| value.parse::<f64>().ok())
}

fn extract_aspect_ratio_from_action_input(raw: &str) -> Option<String> {
    extract_raw_action_scalar(
        raw,
        &[
            "'aspect_ratio'",
            "\"aspect_ratio\"",
            "'aspectRatio'",
            "\"aspectRatio\"",
            "'aspect'",
            "\"aspect\"",
        ],
    )
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
}

pub fn preferred_app_endpoint_action_hints(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<GeminiCanvasProgramActionHints> {
    let expected_action = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Music => "music_generation",
        gemini_canvas::GeminiCanvasMediaOperation::Video => "video_generation",
        gemini_canvas::GeminiCanvasMediaOperation::Image => "image_generation",
    };
    let action = config
        .app_endpoint
        .canvas_program_action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if action != expected_action {
        return None;
    }
    let action_input = config
        .app_endpoint
        .canvas_program_action_input
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(GeminiCanvasProgramActionHints {
        prompt: extract_prompt_from_action_input(action_input),
        duration_seconds: extract_duration_seconds_from_action_input(action_input),
        aspect_ratio: extract_aspect_ratio_from_action_input(action_input),
    })
}

pub fn preferred_app_endpoint_action_prompt(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<String> {
    preferred_app_endpoint_action_hints(operation, config).and_then(|hints| hints.prompt)
}

pub fn preferred_app_endpoint_action_aspect_ratio(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<String> {
    preferred_app_endpoint_action_hints(operation, config).and_then(|hints| hints.aspect_ratio)
}
