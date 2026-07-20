use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini::canvas_program_web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::routing::candidate::ProviderExecutionMode;

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas program web reverse modular adapters target the future Canvas program quota lane and are not yet supported by the generic request planner.",
    )
    .with_code("unsupported_gemini_canvas_program_web_reverse_modular_request_plan")
}

pub fn relay_config_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<surface::GeminiCanvasProgramRelayConfig, GatewayError> {
    surface::relay_config_from_payload(payload)
}

pub fn force_program_owned_payload(payload: &ProviderAccountPayload) -> ProviderAccountPayload {
    let mut cloned = payload.clone();
    let extra_body = cloned
        .extra_body
        .get_or_insert_with(std::collections::HashMap::new);
    extra_body
        .entry("pureHttpMode".to_string())
        .or_insert_with(|| Value::String("preferred".to_string()));
    extra_body.insert(
        "canvasExecutionOwner".to_string(),
        Value::String("program_owned_relay".to_string()),
    );
    extra_body.insert(
        "canvasQuotaMode".to_string(),
        Value::String("canvas_program".to_string()),
    );
    cloned.execution_mode = Some(ProviderExecutionMode::BrowserBacked);
    cloned
}
