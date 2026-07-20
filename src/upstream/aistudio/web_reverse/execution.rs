use std::collections::HashMap;
use std::time::Duration;

use futures::Stream;
use rquest::Method;
use serde_json::Value;
use tracing::info;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::aistudio_web;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::protocol::gemini_canvas;
use crate::protocol::tool_inject;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::aistudio::common::pure_http_material::AIStudioPureHttpMaterial;
use crate::upstream::aistudio::common::request_plan::{
    build_embeddings_request_plan, build_generate_content_request_plan,
};
use crate::upstream::aistudio::common::runtime_material::AIStudioWebReverseRuntimeMaterial;
use crate::upstream::aistudio::common::text_replay::{
    build_code_assistant_offline_request_body, build_program_owned_text_replay_plan,
    build_stream_code_assistant_offline_generation_request_body,
    build_text_only_generate_content_response, extract_code_assistant_generation_id,
    extract_final_text_from_code_assistant_stream,
};
use crate::upstream::aistudio_web as aistudio_web_reverse_modular;
use crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes;
use crate::upstream::client::UpstreamClient;
use crate::upstream::gemini::api as gemini_api_modular;
use crate::upstream::gemini::web_reverse as gemini_web_reverse_modular;
use crate::upstream::response_types::BinaryUpstreamResponse;

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_web(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        if !aistudio_web::supports_text_endpoint(req.endpoint_kind) {
            return Err(
                GatewayError::bad_request(
                    "AI Studio Web reverse currently supports only chat/messages/responses/completions text requests.",
                )
                .with_provider(provider)
                .with_code("unsupported_aistudio_web_reverse_endpoint"),
            );
        }

        let config = aistudio_web_reverse_modular::config_from_payload(payload)?;
        if config.fixture_transport {
            if let Some(canonical) = aistudio_web::build_fixture_canonical_response(req, model) {
                return Ok(canonical);
            }
        }
        let injected_request;
        let request_for_upstream = if !config.fixture_transport && !req.tools.is_empty() {
            let mut cloned = req.clone();
            tool_inject::inject_tools(&mut cloned);
            injected_request = Some(cloned);
            injected_request.as_ref().unwrap()
        } else {
            req
        };
        let request_body = aistudio_web::build_text_request_body(request_for_upstream, model);
        let text_replay_plan = build_program_owned_text_replay_plan(
            aistudio_web::build_generate_content_url(&payload.base_url, model).as_str(),
            &request_body,
        )
        .map_err(|error| error.with_provider(provider))?;
        let response_value = if config.fixture_transport {
            let response_body = self
                .execute_aistudio_fixture_text_request(payload, model, &request_body)
                .await?;
            serde_json::from_str::<Value>(&response_body).map_err(|error| {
                GatewayError::server_error(format!(
                    "AI Studio Web reverse failed to decode generateContent JSON: {error}"
                ))
                .with_provider(provider)
                .with_code("aistudio_web_reverse_invalid_generate_content_json")
            })?
        } else if let Some(response_value) = text_replay_plan.deterministic_response.clone() {
            info!(
                provider = %provider,
                model = %text_replay_plan.model,
                transport_owner = %text_replay_plan.transport_owner.unwrap_or("aistudio_program_owned_text_replay"),
                prompt_len = text_replay_plan.prompt_text.len(),
                code_assistant_path = %text_replay_plan.code_assistant_offline_path,
                stream_path = %text_replay_plan.stream_code_assistant_offline_generation_path,
                "aistudio program-owned deterministic text replay returned synthetic generateContent response"
            );
            response_value
        } else {
            match self
                .execute_aistudio_program_owned_text_request(&config, &text_replay_plan)
                .await
            {
                Ok(response_value) => response_value,
                Err(error) => {
                    info!(
                        provider = %provider,
                        model = %text_replay_plan.model,
                        error_code = ?error.code,
                        error = %error,
                        "aistudio default program-owned pure-http text replay failed; falling back to browser-backed request"
                    );
                    let response_body = self
                        .execute_aistudio_browser_text_request(
                            provider_account_id,
                            payload,
                            &config,
                            req.endpoint_kind,
                            model,
                            &request_body,
                        )
                        .await?;
                    serde_json::from_str::<Value>(&response_body).map_err(|error| {
                        GatewayError::server_error(format!(
                            "AI Studio Web reverse failed to decode generateContent JSON: {error}"
                        ))
                        .with_provider(provider)
                        .with_code("aistudio_web_reverse_invalid_generate_content_json")
                    })?
                }
            }
        };
        let mut canonical =
            gemini_api_modular::parse_generate_content_response(&response_value, model)
                .map_err(|error| error.with_provider(provider))?;
        if canonical.tool_calls.is_empty() && !req.tools.is_empty() {
            let conversation_hint = gemini_canvas::latest_nonempty_user_text(req).or_else(|| {
                let text = req.messages_text().trim().to_string();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            });
            let parsed = tool_inject::parse_tool_calls_from_text_with_context(
                &canonical.text,
                &req.tools,
                req.tool_choice.as_ref(),
                conversation_hint.as_deref(),
            );
            if !parsed.tool_calls.is_empty() {
                canonical.text = parsed.clean_text.trim().to_string();
                canonical.tool_calls = parsed.tool_calls;
                canonical.finish_reason = Some("tool_calls".to_string());
            }
        }
        Ok(canonical)
    }

    async fn execute_aistudio_program_owned_text_request(
        &self,
        config: &aistudio_web::AIStudioWebConfig,
        text_replay_plan: &crate::upstream::aistudio::common::text_replay::AIStudioProgramOwnedTextReplayPlan,
    ) -> Result<Value, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let runtime = AIStudioWebReverseRuntimeMaterial::from(config);
        let Some((contract, material)) = AIStudioPureHttpMaterial::load_from_runtime(&runtime)
            .await
            .map_err(|error| error.with_provider(provider))?
        else {
            return Err(
                GatewayError::service_unavailable(
                    "AI Studio program-owned pure HTTP text replay requires a bound target RPC contract sidecar.",
                )
                .with_provider(provider)
                .with_code("aistudio_program_owned_target_rpc_contract_unavailable"),
            );
        };

        let app_id = contract
            .app_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                GatewayError::service_unavailable(
                    "AI Studio target RPC contract did not include appId for program-owned text replay.",
                )
                .with_provider(provider)
                .with_code("aistudio_program_owned_target_rpc_missing_app_id")
            })?;
        let opaque_token = contract
            .code_assistant_opaque_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                GatewayError::service_unavailable(
                    "AI Studio target RPC contract did not include the CodeAssistant opaque token required for program-owned text replay.",
                )
                .with_provider(provider)
                .with_code("aistudio_program_owned_target_rpc_missing_opaque_token")
            })?;
        let model_path = contract
            .model_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("models/{}", text_replay_plan.model));
        let code_assistant_body = build_code_assistant_offline_request_body(
            &text_replay_plan.prompt_text,
            &model_path,
            app_id,
            opaque_token,
        );
        let code_assistant_headers = material
            .build_capture_aligned_headers(None)
            .map_err(|error| error.with_provider(provider))?;
        let code_assistant_response_body = self
            .execute_aistudio_program_owned_target_rpc(
                provider,
                material.request_url.as_str(),
                code_assistant_headers.clone(),
                &code_assistant_body,
            )
            .await?;
        let generation_id = extract_code_assistant_generation_id(&code_assistant_response_body)
            .ok_or_else(|| {
                GatewayError::service_unavailable(
                    "AI Studio CodeAssistantOffline response did not contain a generationId.",
                )
                .with_provider(provider)
                .with_code("aistudio_program_owned_target_rpc_missing_generation_id")
            })?;
        let stream_url = contract
            .stream_code_assistant_offline_generation
            .url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                replace_url_path(
                    material.request_url.as_str(),
                    text_replay_plan.stream_code_assistant_offline_generation_path,
                )
            });
        let stream_body =
            build_stream_code_assistant_offline_generation_request_body(&generation_id, app_id);
        let stream_response_body = self
            .execute_aistudio_program_owned_target_rpc(
                provider,
                stream_url.as_str(),
                code_assistant_headers,
                &stream_body,
            )
            .await?;
        let final_text = extract_final_text_from_code_assistant_stream(&stream_response_body)
            .ok_or_else(|| {
                GatewayError::service_unavailable(
                    "AI Studio StreamCodeAssistantOfflineGeneration response did not contain a final model text segment.",
                )
                .with_provider(provider)
                .with_code("aistudio_program_owned_target_rpc_missing_final_text")
            })?;
        info!(
            provider = %provider,
            model = %text_replay_plan.model,
            app_id,
            generation_id = %generation_id,
            transport_owner = "aistudio_program_owned_capture_contract_replay",
            "aistudio program-owned pure-http text replay returned final text from target RPC chain"
        );
        Ok(build_text_only_generate_content_response(
            &final_text,
            &text_replay_plan.model,
        ))
    }

    async fn execute_aistudio_program_owned_target_rpc(
        &self,
        provider: &str,
        url: &str,
        headers: rquest::header::HeaderMap,
        body: &Value,
    ) -> Result<String, GatewayError> {
        let response = self
            .http
            .request(Method::POST, url)
            .headers(headers)
            .timeout(self.timeout.max(Duration::from_secs(90)))
            .body(body.to_string())
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let status = response.status().as_u16();
        let body_text: String = response.text().await.map_err(|error| {
            GatewayError::server_error(format!(
                "AI Studio program-owned target RPC body read failed: {error}"
            ))
            .with_provider(provider)
            .with_code("aistudio_program_owned_target_rpc_body_read_failed")
        })?;
        if status >= 400 {
            return Err(classify_upstream_error(status, &body_text, Some(provider)));
        }
        Ok(body_text)
    }

    pub(crate) async fn execute_aistudio_web_stream(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<
        std::pin::Pin<Box<dyn Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
        GatewayError,
    > {
        let canonical = self
            .execute_aistudio_web(provider_account_id, payload, req, model, None)
            .await?;
        let sse_bytes = canonical_response_to_openai_sse_bytes(req, model, &canonical)
            .into_iter()
            .map(Ok);
        Ok(Box::pin(futures::stream::iter(sse_bytes)))
    }

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

    pub(crate) async fn execute_aistudio_web_embeddings(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER;
        let config = aistudio_web_reverse_modular::config_from_payload(payload)?;
        if config.fixture_transport {
            return aistudio_web::build_fixture_embeddings_response(req, model)
                .map_err(|error| error.with_provider(provider));
        }

        let material = AIStudioWebReverseRuntimeMaterial::from(&config);
        let cloud_api_key = material
            .require_cloud_api_key(
                "AI Studio Web reverse embeddings require a cloudApiKey/browser-backed cloud project key.",
                "missing_aistudio_cloud_api_key",
            )
            .map_err(|error| error.with_provider(provider))?;
        let embeddings_request =
            aistudio_web::build_embeddings_request(req, &payload.base_url, model)
                .map_err(|error| error.with_provider(provider))?;
        let request_plan = build_embeddings_request_plan(
            embeddings_request,
            HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
                ("x-goog-api-key".to_string(), cloud_api_key.to_string()),
            ]),
        );
        let request_spec = request_plan.to_browser_request_spec();
        let response_body = self
            .execute_aistudio_browser_request(
                provider_account_id,
                payload,
                &config,
                EndpointKind::Embeddings,
                &request_spec,
            )
            .await?;
        let response_value: Value = serde_json::from_str(&response_body).map_err(|error| {
            GatewayError::server_error(format!(
                "AI Studio Web reverse failed to decode embeddings JSON: {error}"
            ))
            .with_provider(provider)
            .with_code("aistudio_web_reverse_invalid_embeddings_json")
        })?;
        aistudio_web::bridge_embeddings_response(req, model, &response_value)
            .map_err(|error| error.with_provider(provider))
    }
}

fn replace_url_path(url: &str, path: &str) -> String {
    let trimmed = url.trim();
    let Some((origin, _)) = trimmed.split_once("/$rpc/") else {
        return format!("{}{}", trimmed.trim_end_matches('/'), path);
    };
    format!("{origin}{path}")
}
