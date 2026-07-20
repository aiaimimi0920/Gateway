use std::collections::HashMap;

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini::canvas_web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::routing::candidate::ProviderExecutionMode;

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas Web reverse modular adapters use browser-owned Canvas relay execution and are not supported by the generic request planner.",
    )
    .with_code("unsupported_gemini_canvas_web_reverse_modular_request_plan")
}

pub fn relay_config_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<surface::GeminiCanvasBrowserRelayConfig, GatewayError> {
    surface::relay_config_from_payload(payload)
}

pub fn force_browser_owned_payload(payload: &ProviderAccountPayload) -> ProviderAccountPayload {
    let mut cloned = payload.clone();
    let extra_body = cloned.extra_body.get_or_insert_with(HashMap::new);
    extra_body.insert(
        "pureHttpMode".to_string(),
        Value::String("disabled".to_string()),
    );
    extra_body.insert(
        "canvasExecutionOwner".to_string(),
        Value::String("browser_owned_relay".to_string()),
    );
    cloned.execution_mode = Some(ProviderExecutionMode::BrowserBacked);
    cloned
}
