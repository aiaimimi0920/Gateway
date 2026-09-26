//! Resolve image generation output through the direct HTTP lane and browser fallback.

use std::collections::HashMap;
use std::time::Duration;

use rquest::Method;
use serde_json::Value;
use tracing::info;

use crate::error::GatewayError;
use crate::protocol::aistudio_web;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::aistudio::common::request_plan::build_generate_content_request_plan;
use crate::upstream::aistudio::common::runtime_material::AIStudioWebReverseRuntimeMaterial;
use crate::upstream::client::UpstreamClient;
use crate::upstream::gemini::web_reverse as gemini_web_reverse_modular;

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_web_images(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let config = aistudio_web::config_from_payload(payload)?;
        let prompt = gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
            req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )
        .map_err(|error| error.with_provider(provider))?;

        if req.endpoint_kind == EndpointKind::ImagesEdits {
            return Err(GatewayError::bad_request(
                "AI Studio Web reverse image edits are not enabled on this surface yet.",
            )
            .with_provider(provider)
            .with_code("unsupported_aistudio_web_reverse_image_edit"));
        }

        if config.fixture_transport {
            let image =
                aistudio_web::fixture_image().map_err(|error| error.with_provider(provider))?;
            let image = gemini_canvas::GeminiCanvasImage {
                mime_type: image.mime_type,
                bytes: image.bytes,
            };
            return gemini_canvas::build_openai_images_response_from_bytes(req, &prompt, &[image])
                .map_err(|error| error.with_provider(provider));
        }

        let material = AIStudioWebReverseRuntimeMaterial::from(&config);
        let cloud_api_key = material
            .require_cloud_api_key(
                "AI Studio Web reverse live image requests require a cloudApiKey or apiKey in provider payload.",
                "missing_aistudio_web_live_cloud_api_key",
            )
            .map_err(|error| error.with_provider(provider))?;
        let upstream_model = if model.trim().is_empty() {
            "gemini-2.5-flash-image-preview"
        } else {
            model
        };
        let request_plan = build_generate_content_request_plan(
            &payload.base_url,
            upstream_model,
            HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
                ("x-goog-api-key".to_string(), cloud_api_key.to_string()),
            ]),
            aistudio_web::build_image_request_body(req, upstream_model),
        );
        let request_url = request_plan.url.clone();
        let request_spec = request_plan.to_browser_request_spec();
        let direct_timeout = self.timeout.max(Duration::from_secs(180));
        let response_body = match self
            .http
            .request(Method::POST, &request_url)
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .header("x-goog-api-key", cloud_api_key)
            .timeout(direct_timeout)
            .json(&request_plan.body)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.map_err(|error| {
                    GatewayError::server_error(format!(
                        "AI Studio Web reverse direct image response body read failed: {error}"
                    ))
                    .with_provider(provider)
                    .with_code("aistudio_web_reverse_images_direct_body_read_failed")
                })?;
                info!(
                    provider = %provider,
                    model = %upstream_model,
                    body_len = body.len(),
                    "aistudio live images direct-http fast path returned body"
                );
                body
            }
            Ok(response) => {
                let status = response.status().as_u16();
                let preview = response.text().await.unwrap_or_default();
                info!(
                    provider = %provider,
                    model = %upstream_model,
                    status,
                    preview = %preview.chars().take(240).collect::<String>(),
                    "aistudio live images direct-http fast path failed; falling back to browser-backed request"
                );
                self.execute_aistudio_browser_request(
                    provider_account_id,
                    payload,
                    &config,
                    EndpointKind::ImagesGenerations,
                    &request_spec,
                )
                .await?
            }
            Err(error) => {
                info!(
                    provider = %provider,
                    model = %upstream_model,
                    error = %error,
                    "aistudio live images direct-http fast path transport failed; falling back to browser-backed request"
                );
                self.execute_aistudio_browser_request(
                    provider_account_id,
                    payload,
                    &config,
                    EndpointKind::ImagesGenerations,
                    &request_spec,
                )
                .await?
            }
        };

        let response_value: Value = serde_json::from_str(&response_body).map_err(|error| {
            GatewayError::server_error(format!(
                "AI Studio Web reverse failed to decode image JSON: {error}"
            ))
            .with_provider(provider)
            .with_code("aistudio_web_reverse_invalid_image_json")
        })?;
        let image =
            gemini_canvas::extract_inline_image_from_generate_content_response(&response_value)
                .map_err(|error| error.with_provider(provider))?;
        gemini_canvas::build_openai_images_response_from_bytes(req, &prompt, &[image])
            .map_err(|error| error.with_provider(provider))
    }
}
