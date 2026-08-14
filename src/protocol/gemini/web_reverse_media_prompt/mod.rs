mod extract;
mod guidance;
mod matching;

#[cfg(test)]
mod tests;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas::{aspect_ratio_from_request, GeminiCanvasMediaOperation};

pub fn prompt_for_media_request(
    req: &CanonicalRelayRequest,
    operation: GeminiCanvasMediaOperation,
) -> Result<String, GatewayError> {
    let (missing_message, missing_code) = match operation {
        GeminiCanvasMediaOperation::Image => (
            "Gemini Canvas image requests require a prompt.",
            "missing_prompt",
        ),
        GeminiCanvasMediaOperation::Music => (
            "Gemini Canvas music requests require a prompt.",
            "missing_music_prompt",
        ),
        GeminiCanvasMediaOperation::Video => (
            "Gemini Canvas video requests require a prompt.",
            "missing_video_prompt",
        ),
    };

    let mut prompt = extract::prompt_from_request(req, missing_message, missing_code)?;
    if operation == GeminiCanvasMediaOperation::Music {
        return Ok(format!(
            "Create an original music clip that matches this request. Return the music generation directly instead of explaining the request in text.\n\nMusic request:\n{prompt}"
        ));
    }

    let guidance = guidance::media_prompt_guidance_lines(req, operation);
    if !guidance.is_empty() {
        let guidance_block = format!("Generation guidance:\n- {}", guidance.join("\n- "));
        if !matching::prompt_contains_guidance_block(&prompt, &guidance_block) {
            prompt = format!("{guidance_block}\n\n{prompt}");
        }
    }

    let has_explicit_aspect_ratio = req
        .raw_body
        .get("size")
        .or_else(|| req.raw_body.get("aspect_ratio"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    let aspect_ratio =
        if operation == GeminiCanvasMediaOperation::Video && !has_explicit_aspect_ratio {
            "16:9".to_string()
        } else {
            aspect_ratio_from_request(req)
        };
    if matches!(
        operation,
        GeminiCanvasMediaOperation::Image | GeminiCanvasMediaOperation::Video
    ) && aspect_ratio != "auto"
        && !matching::prompt_contains_aspect_ratio(&prompt, &aspect_ratio)
    {
        return Ok(format!(
            "{prompt}\n\nRequested aspect ratio: {aspect_ratio}."
        ));
    }

    Ok(prompt)
}
