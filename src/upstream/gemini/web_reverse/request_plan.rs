use crate::error::GatewayError;

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Web modular adapters use app bootstrap + reverse-web replay and are not supported by the generic request planner.",
    )
    .with_code("unsupported_gemini_web_modular_request_plan")
}
