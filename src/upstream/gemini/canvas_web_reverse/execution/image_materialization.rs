//! Select, decode, and materialize image assets without changing download order.

use std::time::Duration;

use base64::Engine;
use rquest::{Client, Method};
use serde_json::Value;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas;
use crate::upstream::gemini::canvas_program_web_reverse as program;
use crate::upstream::gemini_canvas_followup_types::{
    plan_gemini_canvas_image_response, GeminiCanvasImageResponsePlan,
};

pub fn decode_inline_image_asset(
    provider: &str,
    asset: &gemini_canvas::GeminiCanvasMediaAsset,
    invalid_inline_code: &'static str,
    context_label: &str,
) -> Result<Option<gemini_canvas::GeminiCanvasImage>, GatewayError> {
    let Some(body_base64) = asset.body_base64.as_deref() else {
        return Ok(None);
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(body_base64)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "{context_label} returned invalid inline image bytes: {error}"
            ))
            .with_provider(provider)
            .with_code(invalid_inline_code)
        })?;

    Ok(Some(gemini_canvas::GeminiCanvasImage {
        mime_type: asset.mime_type.clone(),
        bytes,
    }))
}

pub fn browser_pool_missing_image_asset_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas browser pool completed without returning an image asset.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_no_image_asset")
}

pub(crate) fn plan_direct_http_image_response<'a>(
    req: &CanonicalRelayRequest,
    prompt: &str,
    image_assets: &'a [gemini_canvas::GeminiCanvasMediaAsset],
    provider: &str,
) -> Result<GeminiCanvasImageResponsePlan<'a>, GatewayError> {
    plan_gemini_canvas_image_response(
        req,
        prompt,
        image_assets,
        provider,
        "Gemini Canvas direct HTTP image generation completed without returning an image asset.",
        "gemini_canvas_no_image_asset",
    )
}

pub fn decode_browser_pool_inline_image_asset(
    provider: &str,
    asset: &gemini_canvas::GeminiCanvasMediaAsset,
) -> Result<Option<gemini_canvas::GeminiCanvasImage>, GatewayError> {
    decode_inline_image_asset(
        provider,
        asset,
        "gemini_canvas_invalid_inline_image_bytes",
        "Gemini Canvas browser pool",
    )
}

pub(crate) fn decode_direct_http_inline_image_asset(
    provider: &str,
    asset: &gemini_canvas::GeminiCanvasMediaAsset,
) -> Result<Option<gemini_canvas::GeminiCanvasImage>, GatewayError> {
    decode_inline_image_asset(
        provider,
        asset,
        "gemini_canvas_invalid_inline_image_bytes",
        "Gemini Canvas direct HTTP image asset contained invalid base64 bytes",
    )
}

pub fn select_downloaded_image_mime_type(
    fallback_mime_type: &str,
    response_content_type: Option<&str>,
) -> String {
    response_content_type
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(ToString::to_string)
        .unwrap_or_else(|| fallback_mime_type.to_string())
}

pub fn build_downloaded_image_from_bytes(
    asset: &gemini_canvas::GeminiCanvasMediaAsset,
    response_content_type: Option<&str>,
    bytes: &[u8],
) -> gemini_canvas::GeminiCanvasImage {
    gemini_canvas::GeminiCanvasImage {
        mime_type: select_downloaded_image_mime_type(&asset.mime_type, response_content_type),
        bytes: bytes.to_vec(),
    }
}

async fn download_image_asset_from_url(
    http: &Client,
    provider: &str,
    asset: &gemini_canvas::GeminiCanvasMediaAsset,
    request_timeout: Duration,
) -> Result<gemini_canvas::GeminiCanvasImage, GatewayError> {
    let response = http
        .request(Method::GET, &asset.url)
        .timeout(request_timeout)
        .redirect(rquest::redirect::Policy::limited(10))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("<unreadable body>"));
        return Err(crate::error::classify_upstream_error(
            status,
            &body_text,
            Some(provider),
        ));
    }

    let response_content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let bytes = response
        .bytes()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;

    Ok(build_downloaded_image_from_bytes(
        asset,
        response_content_type.as_deref(),
        bytes.as_ref(),
    ))
}

async fn materialize_image_asset(
    http: &Client,
    provider: &str,
    asset: &gemini_canvas::GeminiCanvasMediaAsset,
    request_timeout: Duration,
) -> Result<gemini_canvas::GeminiCanvasImage, GatewayError> {
    if let Some(image) = decode_inline_image_asset(
        provider,
        asset,
        "gemini_canvas_modular_invalid_inline_image_bytes",
        "Gemini Canvas modular browser relay",
    )? {
        return Ok(image);
    }

    download_image_asset_from_url(http, provider, asset, request_timeout).await
}

pub async fn build_image_generation_response_from_invocation(
    http: &Client,
    provider: &str,
    req: &CanonicalRelayRequest,
    prompt: &str,
    result: &program::GeminiCanvasBrowserInvocationResult,
    request_timeout: Duration,
) -> Result<Value, GatewayError> {
    let image_assets = super::super::result::collect_converted_image_media_assets(result);
    if image_assets.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas modular browser relay completed without returning an image asset.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_modular_no_image_asset"));
    }

    if gemini_canvas::prefers_url_response(req)? {
        return gemini_canvas::build_openai_images_response_from_urls(req, prompt, &image_assets);
    }

    let mut downloaded = Vec::with_capacity(image_assets.len());
    for asset in image_assets
        .iter()
        .take(gemini_canvas::requested_output_count(req))
    {
        downloaded.push(materialize_image_asset(http, provider, asset, request_timeout).await?);
    }

    gemini_canvas::build_openai_images_response_from_bytes(req, prompt, &downloaded)
}
