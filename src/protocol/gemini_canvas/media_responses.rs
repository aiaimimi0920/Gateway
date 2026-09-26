use serde_json::Value;

use super::types::GeminiCanvasMediaAsset;

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    asset: &GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_music_generation_response(
        model, prompt, asset, body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

pub fn build_music_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    duration_seconds: Option<f64>,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_music_generation_accepted_response(
        model,
        prompt,
        conversation_id,
        response_id,
        app_path,
        duration_seconds,
        body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    asset: &GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_video_generation_response(
        model, prompt, asset, body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

pub fn build_video_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    job_id: Option<&str>,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_video_generation_accepted_response(
        model,
        prompt,
        conversation_id,
        response_id,
        app_path,
        job_id,
        body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

pub fn video_body_indicates_music_modality_mismatch(body_text: &str) -> bool {
    let lower = body_text.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }

    lower.contains("generated_music_content")
        || lower.contains("create an original music clip")
        || lower.contains("music request:")
        || lower.contains("gemini.google.com/music")
        || lower.contains("\"object\":\"music.generation\"")
}
