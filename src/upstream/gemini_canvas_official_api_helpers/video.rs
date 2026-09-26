use std::collections::HashMap;
use std::time::{Duration, Instant};

use base64::Engine;
use rquest::Client;
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::web_reverse as gemini_web_reverse_modular;
use crate::upstream::gemini_canvas_followup_types::{
    extract_gemini_canvas_video_operation_download_uri,
    gemini_canvas_video_missing_operation_error, gemini_canvas_video_operation_timeout_error,
    gemini_canvas_video_unsupported_count_error,
};

use super::{
    send_gemini_canvas_official_get_bytes, send_gemini_canvas_official_get_json,
    send_gemini_canvas_official_json,
};

pub(crate) async fn execute_gemini_canvas_official_video(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    request_url_override: Option<&str>,
    prompt_override: Option<&str>,
    aspect_ratio_override: Option<&str>,
    duration_seconds_override: Option<f64>,
    upstream_model_override: Option<&str>,
) -> Result<Value, GatewayError> {
    let upstream_model_owned;
    let upstream_model = if let Some(override_model) = upstream_model_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        upstream_model_owned = override_model.to_string();
        upstream_model_owned.as_str()
    } else {
        gemini_canvas::resolve_official_video_model(model)?
    };
    if gemini_canvas::requested_output_count(req) > 1 {
        return Err(gemini_canvas_video_unsupported_count_error(
            "gemini_canvas_compatible",
        ));
    }
    let prompt = prompt_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or(
            gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                req,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
            )?,
        );
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
    let resolved_aspect_ratio =
        if req.raw_body.get("size").is_some() || req.raw_body.get("aspect_ratio").is_some() {
            gemini_canvas::aspect_ratio_from_request(req)
        } else {
            aspect_ratio_override
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| gemini_canvas::aspect_ratio_from_request(req))
        };
    let mut parameters = serde_json::Map::new();
    parameters.insert("aspectRatio".to_string(), json!(resolved_aspect_ratio));
    if let Some(duration_seconds) = duration_seconds_override {
        if req.raw_body.get("duration_s").is_none()
            && req.raw_body.get("durationSeconds").is_none()
            && req.raw_body.get("duration").is_none()
        {
            parameters.insert("durationSeconds".to_string(), json!(duration_seconds));
        }
    }
    let request_body = json!({
        "instances": [{"prompt": prompt}],
        "parameters": parameters
    });
    let operation = send_gemini_canvas_official_json(
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
        .ok_or_else(|| gemini_canvas_video_missing_operation_error("gemini_canvas_compatible"))?;
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

    let deadline = Instant::now() + timeout.max(Duration::from_secs(600));
    let mut poll = operation;
    while Instant::now() < deadline {
        if poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
            break;
        }
        sleep(Duration::from_secs(5)).await;
        poll = send_gemini_canvas_official_get_json(
            http,
            payload,
            &operation_url,
            timeout.max(Duration::from_secs(30)),
            extra_headers,
        )
        .await?;
    }
    if !poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
        return Err(gemini_canvas_video_operation_timeout_error(
            "gemini_canvas_compatible",
        ));
    }
    if let Some(error) = poll.get("error") {
        return Err(classify_upstream_error(
            502,
            &error.to_string(),
            Some("gemini_canvas_compatible"),
        ));
    }
    let video_uri =
        extract_gemini_canvas_video_operation_download_uri("gemini_canvas_compatible", &poll)?;
    let (bytes, content_type) = send_gemini_canvas_official_get_bytes(
        http,
        payload,
        video_uri,
        timeout.max(Duration::from_secs(120)),
        extra_headers,
    )
    .await?;
    let mime_type = content_type.unwrap_or_else(|| "video/mp4".to_string());
    let body_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let asset = gemini_canvas::GeminiCanvasMediaAsset {
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
    Ok(gemini_canvas::build_video_generation_response(
        model, &prompt, &asset, None,
    ))
}
