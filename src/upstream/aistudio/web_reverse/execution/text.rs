//! Text and synthetic-stream execution with tool injection and browser fallback.

use std::collections::HashMap;

use futures::Stream;
use serde_json::Value;
use tracing::info;

use crate::error::GatewayError;
use crate::protocol::aistudio_web;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::{gemini_canvas, tool_inject};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::aistudio::common::text_replay::build_program_owned_text_replay_plan;
use crate::upstream::aistudio_web as aistudio_web_reverse_modular;
use crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes;
use crate::upstream::client::UpstreamClient;
use crate::upstream::gemini::api as gemini_api_modular;

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
}
