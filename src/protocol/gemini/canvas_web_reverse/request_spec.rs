use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::relay_config::{
    GeminiCanvasBrowserRelayConfig, GEMINI_CANVAS_BROWSER_RELAY_PROVIDER_KEY,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCanvasBrowserRelayRequestSpec {
    pub request_id: String,
    pub request_attempt_id: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

pub fn build_browser_relay_request_spec(
    request_id: impl Into<String>,
    request_attempt_id: impl Into<String>,
    method: impl Into<String>,
    url: Option<String>,
    path: Option<String>,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> GeminiCanvasBrowserRelayRequestSpec {
    GeminiCanvasBrowserRelayRequestSpec {
        request_id: request_id.into(),
        request_attempt_id: request_attempt_id.into(),
        method: method.into(),
        url,
        path,
        headers,
        body,
    }
}

pub fn build_browser_executor_invocation_input(
    relay: &GeminiCanvasBrowserRelayConfig,
    request_spec: &GeminiCanvasBrowserRelayRequestSpec,
) -> Value {
    json!({
        "provider": GEMINI_CANVAS_BROWSER_RELAY_PROVIDER_KEY,
        "input": {
            "runtimeStateObjectKey": relay.runtime_state_object_key,
            "shareId": relay.share_id,
            "relayWsEndpoint": relay.relay_ws_endpoint,
            "clientLabel": relay.client_label,
            "requestSpec": request_spec,
        }
    })
}
