//! Resolve TTS output through the direct HTTP lane and its browser fallback.

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
use crate::upstream::response_types::BinaryUpstreamResponse;

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_web_tts(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let config = aistudio_web::config_from_payload(payload)?;
        if config.fixture_transport {
            let (bytes, content_type) = aistudio_web::build_fixture_audio_binary_response(req)
                .map_err(|error| error.with_provider(provider))?;
            return Ok(BinaryUpstreamResponse {
                body: bytes::Bytes::from(bytes),
                content_type: Some(content_type),
                extra_headers: Vec::new(),
            });
        }

        let material = AIStudioWebReverseRuntimeMaterial::from(&config);
        let cloud_api_key = material
            .require_cloud_api_key(
                "AI Studio Web reverse live TTS requests require a cloudApiKey or apiKey in provider payload.",
                "missing_aistudio_web_live_cloud_api_key",
            )
            .map_err(|error| error.with_provider(provider))?;
        let upstream_model = if model.trim().is_empty() {
            "gemini-2.5-flash-preview-tts"
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
            aistudio_web::build_tts_request_body(req, upstream_model),
        );
        let request_url = request_plan.url.clone();
        let request_spec = request_plan.to_browser_request_spec();
        let direct_timeout = self.timeout.max(Duration::from_secs(120));
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
                        "AI Studio Web reverse direct TTS response body read failed: {error}"
                    ))
                    .with_provider(provider)
                    .with_code("aistudio_web_reverse_tts_direct_body_read_failed")
                })?;
                info!(
                    provider = %provider,
                    model = %upstream_model,
                    body_len = body.len(),
                    "aistudio live tts direct-http fast path returned body"
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
                    "aistudio live tts direct-http fast path failed; falling back to browser-backed request"
                );
                self.execute_aistudio_browser_request(
                    provider_account_id,
                    payload,
                    &config,
                    EndpointKind::AudioSpeech,
                    &request_spec,
                )
                .await?
            }
            Err(error) => {
                info!(
                    provider = %provider,
                    model = %upstream_model,
                    error = %error,
                    "aistudio live tts direct-http fast path transport failed; falling back to browser-backed request"
                );
                self.execute_aistudio_browser_request(
                    provider_account_id,
                    payload,
                    &config,
                    EndpointKind::AudioSpeech,
                    &request_spec,
                )
                .await?
            }
        };
        info!(
            provider = %provider,
            model = %upstream_model,
            body_len = response_body.len(),
            "aistudio live tts browser request returned body"
        );
        let response_value: Value = serde_json::from_str(&response_body).map_err(|error| {
            GatewayError::server_error(format!(
                "AI Studio Web reverse failed to decode TTS JSON: {error}"
            ))
            .with_provider(provider)
            .with_code("aistudio_web_reverse_invalid_tts_json")
        })?;
        info!(
            provider = %provider,
            model = %upstream_model,
            "aistudio live tts JSON parsed"
        );
        let audio = gemini_canvas::extract_audio_from_generate_content_response(&response_value)
            .map_err(|error| error.with_provider(provider))?;
        info!(
            provider = %provider,
            model = %upstream_model,
            mime_type = %audio.mime_type,
            audio_bytes = audio.bytes.len(),
            "aistudio live tts audio extracted"
        );
        let (bytes, content_type) = gemini_canvas::build_audio_binary_response(req, &audio)
            .map_err(|error| error.with_provider(provider))?;
        info!(
            provider = %provider,
            model = %upstream_model,
            content_type = %content_type,
            response_bytes = bytes.len(),
            "aistudio live tts binary audio built"
        );
        Ok(BinaryUpstreamResponse {
            body: bytes::Bytes::from(bytes),
            content_type: Some(content_type),
            extra_headers: Vec::new(),
        })
    }
}
