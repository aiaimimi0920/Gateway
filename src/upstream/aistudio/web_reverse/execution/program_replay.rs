//! Execute the captured CodeAssistant RPC chain using one runtime-material snapshot.

use std::time::Duration;

use rquest::Method;
use serde_json::Value;
use tracing::info;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::aistudio_web;
use crate::upstream::aistudio::common::pure_http_material::AIStudioPureHttpMaterial;
use crate::upstream::aistudio::common::runtime_material::AIStudioWebReverseRuntimeMaterial;
use crate::upstream::aistudio::common::text_replay::{
    build_code_assistant_offline_request_body,
    build_stream_code_assistant_offline_generation_request_body,
    build_text_only_generate_content_response, extract_code_assistant_generation_id,
    extract_final_text_from_code_assistant_stream,
};
use crate::upstream::client::UpstreamClient;

impl UpstreamClient {
    pub(super) async fn execute_aistudio_program_owned_text_request(
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
}

fn replace_url_path(url: &str, path: &str) -> String {
    let trimmed = url.trim();
    let Some((origin, _)) = trimmed.split_once("/$rpc/") else {
        return format!("{}{}", trimmed.trim_end_matches('/'), path);
    };
    format!("{origin}{path}")
}
