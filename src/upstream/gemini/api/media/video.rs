//! Official video operation polling and completed-asset download orchestration.

use std::collections::HashMap;
use std::time::Duration;

use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini::api as surface;
use crate::protocol::gemini_canvas as legacy;
use crate::routing::candidate::ProviderAccountPayload;
use base64::Engine;
use rquest::Client;
use serde_json::{json, Value};
use tokio::time::sleep;

use super::prompt_from_media_request;
use super::transport::{send_official_get_bytes, send_official_get_json, send_official_json};

pub async fn execute_official_video(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    request_url_override: Option<&str>,
) -> Result<Value, GatewayError> {
    let upstream_model = surface::resolve_official_video_model(model)?;
    if surface::requested_output_count(req) > 1 {
        return Err(GatewayError::bad_request(
            "Gemini official video generation currently supports only n=1 requests.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_video_count"));
    }
    let prompt = prompt_from_media_request(
        req,
        "Gemini official video generation requires a prompt.",
        "missing_gemini_official_video_prompt",
    )?;
    let request_url = request_url_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "{}/models/{}:predictLongRunning",
                payload.base_url.trim_end_matches('/'),
                upstream_model
            )
        });
    let request_body = json!({
        "instances": [{
            "prompt": prompt
        }],
        "parameters": {
            "aspectRatio": surface::video_aspect_ratio_from_request(req)
        }
    });
    let operation = send_official_json(
        http,
        payload,
        &request_url,
        &request_body,
        timeout.max(Duration::from_secs(30)),
        extra_headers,
    )
    .await?;
    let operation_name = operation
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini official video generation did not return an operation name.",
            )
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_video_missing_operation")
        })?;
    let operation_url =
        if operation_name.starts_with("http://") || operation_name.starts_with("https://") {
            operation_name.to_string()
        } else {
            format!(
                "{}/{}",
                payload.base_url.trim_end_matches('/'),
                operation_name.trim_start_matches('/')
            )
        };

    let deadline = std::time::Instant::now() + timeout.max(Duration::from_secs(600));
    let mut poll = operation;
    while std::time::Instant::now() < deadline {
        if poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
            break;
        }
        sleep(Duration::from_secs(5)).await;
        poll = send_official_get_json(
            http,
            payload,
            &operation_url,
            timeout.max(Duration::from_secs(30)),
            extra_headers,
        )
        .await?;
    }
    if !poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
        return Err(GatewayError::service_unavailable(
            "Gemini official video generation timed out before the operation completed.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_video_operation_timeout"));
    }
    if let Some(error) = poll.get("error") {
        return Err(classify_upstream_error(
            502,
            &error.to_string(),
            Some(payload.adapter.as_str()),
        ));
    }
    let video = poll
        .pointer("/response/generateVideoResponse/generatedSamples/0/video")
        .or_else(|| poll.pointer("/response/generatedVideos/0/video"))
        .or_else(|| poll.pointer("/response/generated_videos/0/video"))
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini official video operation completed without a downloadable video payload.",
            )
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_no_video_asset")
        })?;
    let video_uri = video
        .get("uri")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini official video operation completed without video.uri.",
            )
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_no_video_asset")
        })?;
    let (bytes, content_type) = send_official_get_bytes(
        http,
        payload,
        video_uri,
        timeout.max(Duration::from_secs(120)),
        extra_headers,
    )
    .await?;
    let mime_type = content_type.unwrap_or_else(|| "video/mp4".to_string());
    let body_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let asset = legacy::GeminiCanvasMediaAsset {
        kind: "video".to_string(),
        url: video_uri.to_string(),
        mime_type,
        download_token: None,
        body_base64: Some(body_base64),
        alt: Some(prompt.clone()),
        width: None,
        height: None,
        duration_seconds: None,
    };
    Ok(surface::build_video_generation_response(
        model, &prompt, &asset, None,
    ))
}
