use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;

pub const GEMINI_CANVAS_BROWSER_RELAY_DEFAULT_WS_PATH: &str = "/ws";
pub const GEMINI_CANVAS_BROWSER_RELAY_PROVIDER_KEY: &str = "gemini_canvas_browser_relay";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasBrowserRelayConfig {
    pub runtime_state_object_key: String,
    pub share_id: String,
    pub relay_ws_endpoint: Option<String>,
    pub client_label: Option<String>,
}

fn read_optional_extra_string(
    extra: Option<&HashMap<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| extra.and_then(|body| body.get(*key)))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn relay_config_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<GeminiCanvasBrowserRelayConfig, GatewayError> {
    let runtime = super::runtime_from_payload(payload)?;
    let extra = payload.extra_body.as_ref();

    Ok(GeminiCanvasBrowserRelayConfig {
        runtime_state_object_key: runtime.runtime_state_object_key,
        share_id: runtime.share_id,
        relay_ws_endpoint: read_optional_extra_string(
            extra,
            &[
                "canvasRelayWsEndpoint",
                "canvas_relay_ws_endpoint",
                "browserRelayWsEndpoint",
                "browser_relay_ws_endpoint",
            ],
        ),
        client_label: read_optional_extra_string(
            extra,
            &[
                "canvasRelayClientLabel",
                "canvas_relay_client_label",
                "browserRelayClientLabel",
                "browser_relay_client_label",
            ],
        ),
    })
}
