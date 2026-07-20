use serde_json::Value;

use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas::GeminiCanvasMediaOperation;

pub(super) fn media_prompt_guidance_lines(
    req: &CanonicalRelayRequest,
    operation: GeminiCanvasMediaOperation,
) -> Vec<String> {
    let mut lines = Vec::new();
    match operation {
        GeminiCanvasMediaOperation::Image => {
            maybe_push_guidance_line(&mut lines, &req.raw_body, &["style"], "Style guidance");
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["quality"],
                "Quality preference",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["background"],
                "Background preference",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["output_format", "format"],
                "Output format preference",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["output_compression"],
                "Output compression preference",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["moderation"],
                "Moderation preference",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["input_fidelity"],
                "Input fidelity preference",
            );
        }
        GeminiCanvasMediaOperation::Video => {
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["negativePrompt", "negative_prompt"],
                "Avoid the following elements",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["durationSeconds", "duration_seconds"],
                "Target duration",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["resolution"],
                "Target resolution",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["personGeneration", "person_generation"],
                "People generation preference",
            );
            maybe_push_guidance_line(
                &mut lines,
                &req.raw_body,
                &["generateAudio", "generate_audio"],
                "Audio generation preference",
            );
        }
        GeminiCanvasMediaOperation::Music => {}
    }
    lines
}

fn maybe_push_guidance_line(lines: &mut Vec<String>, value: &Value, keys: &[&str], label: &str) {
    let Some(raw) = read_optional_scalar_string(value, keys) else {
        return;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    let line = format!("{label}: {trimmed}");
    if lines
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&line))
    {
        return;
    }
    lines.push(line);
}

fn read_optional_scalar_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        let Some(entry) = map.get(*key) else {
            continue;
        };
        let rendered = match entry {
            Value::String(value) => value.trim().to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            _ => continue,
        };
        if !rendered.is_empty() {
            return Some(rendered);
        }
    }
    None
}
