//! Official media endpoint dispatch and shared prompt selection.

use std::collections::HashMap;
use std::time::Duration;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::gemini::api as surface;
use crate::protocol::gemini_canvas as legacy;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::response_types::BinaryUpstreamResponse;
use rquest::Client;
use serde_json::Value;

mod music;
mod transport;
mod video;

pub use music::execute_official_music;
pub use video::execute_official_video;

use transport::send_official_json;

pub fn supports_media_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::AudioSpeech
            | EndpointKind::ImagesGenerations
            | EndpointKind::MusicGenerations
            | EndpointKind::VideosGenerations
    )
}

pub async fn execute_official_tts(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    let upstream_model = if model.trim().is_empty() {
        legacy::GEMINI_CANVAS_OFFICIAL_TTS_MODEL
    } else {
        model
    };
    let request_body = surface::build_tts_request_body(req, upstream_model);
    let request_url = format!(
        "{}/models/{}:generateContent",
        payload.base_url.trim_end_matches('/'),
        upstream_model
    );
    let body = send_official_json(
        http,
        payload,
        &request_url,
        &request_body,
        timeout.max(Duration::from_secs(120)),
        extra_headers,
    )
    .await?;
    let audio = surface::extract_audio_from_generate_content_response(&body)?;
    let (bytes, content_type) = surface::build_audio_binary_response(req, &audio)?;
    Ok(BinaryUpstreamResponse {
        body: bytes::Bytes::from(bytes),
        content_type: Some(content_type),
        extra_headers: Vec::new(),
    })
}

pub async fn execute_official_media(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    match req.endpoint_kind {
        EndpointKind::ImagesGenerations => {
            execute_official_image(http, timeout, payload, req, model, extra_headers).await
        }
        EndpointKind::MusicGenerations => {
            execute_official_music(http, timeout, payload, req, model, extra_headers, None).await
        }
        EndpointKind::VideosGenerations => {
            execute_official_video(http, timeout, payload, req, model, extra_headers, None).await
        }
        EndpointKind::ImagesEdits => Err(GatewayError::bad_request(
            "Gemini official API does not support image edits on this compatibility surface yet.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_edit_endpoint")),
        _ => Err(GatewayError::bad_request(
            "Gemini official API adapters currently support only /v1/images/generations, /v1/music/generations, and /v1/videos/generations for media endpoints.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_media_endpoint")),
    }
}

pub async fn execute_official_image(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let upstream_model = surface::resolve_official_image_model(model)?;
    if surface::requested_output_count(req) > 1 {
        return Err(GatewayError::bad_request(
            "Gemini official image generation currently supports only n=1 requests.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_image_count"));
    }
    let prompt = prompt_from_media_request(
        req,
        "Gemini official image generation requires a prompt.",
        "missing_gemini_official_image_prompt",
    )?;
    let request_body = surface::build_image_request_body(req, upstream_model);
    let request_url = format!(
        "{}/models/{}:generateContent",
        payload.base_url.trim_end_matches('/'),
        upstream_model
    );
    let body = send_official_json(
        http,
        payload,
        &request_url,
        &request_body,
        timeout.max(Duration::from_secs(120)),
        extra_headers,
    )
    .await?;
    let image = surface::extract_inline_image_from_generate_content_response(&body)?;
    surface::build_openai_images_response_from_bytes(req, &prompt, &[image])
}

fn prompt_from_media_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    if let Some(prompt) = req
        .raw_body
        .get("prompt")
        .or_else(|| req.raw_body.get("input"))
        .or_else(|| req.raw_body.get("lyrics"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(prompt.to_string());
    }
    let prompt = req.messages_text().trim().to_string();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(missing_message).with_code(missing_code));
    }
    Ok(prompt)
}
