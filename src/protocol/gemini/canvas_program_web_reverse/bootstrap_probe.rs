use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;

use super::relay_config::relay_config_from_payload;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCanvasProgramBootstrapProbe {
    pub share_url: String,
    pub app_url: String,
    pub expect_canvas_program: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

pub fn build_program_bootstrap_probe(
    payload: &ProviderAccountPayload,
) -> Result<GeminiCanvasProgramBootstrapProbe, GatewayError> {
    let config = relay_config_from_payload(payload)?;
    let share_base = payload.base_url.trim_end_matches('/');
    let share_url = format!("{share_base}/share/{}", config.bootstrap.share_id);
    let app_url = config
        .app_endpoint
        .canvas_program_url
        .clone()
        .unwrap_or_else(|| format!("{share_base}/app"));

    let mut metadata = HashMap::new();
    if let Some(label) = config.bootstrap.client_label.as_ref() {
        metadata.insert("clientLabel".to_string(), label.clone());
    }
    if let Some(endpoint) = config.bootstrap.relay_ws_endpoint.as_ref() {
        metadata.insert("relayWsEndpoint".to_string(), endpoint.clone());
    }
    if let Some(hint) = config.bootstrap.canvas_program_hint.as_ref() {
        metadata.insert("canvasProgramHint".to_string(), hint.clone());
    }
    if let Some(app_path) = config.app_endpoint.app_path.as_ref() {
        metadata.insert("appPath".to_string(), app_path.clone());
    }
    if let Some(conversation_id) = config.app_endpoint.conversation_id.as_ref() {
        metadata.insert("conversationId".to_string(), conversation_id.clone());
    }
    if let Some(response_id) = config.app_endpoint.response_id.as_ref() {
        metadata.insert("responseId".to_string(), response_id.clone());
    }
    metadata.insert(
        "runtimeStateObjectKey".to_string(),
        config.bootstrap.runtime_state_object_key.clone(),
    );

    Ok(GeminiCanvasProgramBootstrapProbe {
        share_url,
        app_url,
        expect_canvas_program: true,
        metadata,
    })
}
