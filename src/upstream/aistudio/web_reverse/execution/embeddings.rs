//! Build browser-backed embedding requests and bridge their responses.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::aistudio_web;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::aistudio::common::request_plan::build_embeddings_request_plan;
use crate::upstream::aistudio::common::runtime_material::AIStudioWebReverseRuntimeMaterial;
use crate::upstream::aistudio_web as aistudio_web_reverse_modular;
use crate::upstream::client::UpstreamClient;

impl UpstreamClient {
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
