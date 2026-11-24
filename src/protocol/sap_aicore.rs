//! SAP AI Core 的显式推理部署；服务密钥与短期 Token 各自保留生命周期。
pub mod auth;
mod bedrock;
mod bedrock_images;
mod bedrock_tools;
mod config;
pub(crate) mod deployments;
pub(crate) mod errors;
mod fixture;
mod image_input;
mod mtls;
pub(crate) mod native_responses;
pub(crate) mod orchestration;
mod orchestration_stream;
pub(crate) mod probe;
pub(crate) mod probe_evidence;
mod reasoning;
pub(crate) mod responses_lifecycle;
pub mod runtime;
pub(crate) mod sender;
mod service_key;
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
pub use config::{config_binding, validate, validate_declaration, Endpoint};
pub(crate) use config::{
    IMAGE_INPUT, NATIVE_RESPONSES, NATIVE_RESPONSES_IMAGE_INPUT, O3_MINI, STORED_RESPONSES,
};
pub use service_key::ServiceKey;
pub(crate) const BEDROCK_CONVERSE: &str = "sap_bedrock_converse";
pub(crate) const BEDROCK_IMAGE_INPUT: &str = "sap_bedrock_image_input";
pub(crate) const ORCHESTRATION_REASONING: &str = "sap_orchestration_reasoning";
pub(crate) const ORCHESTRATION_IMAGE_INPUT: &str = "sap_orchestration_image_input";
pub(crate) const ORCHESTRATION_MODEL: &str = "sap_orchestration_model";
pub const PRESET: &str = "sap-aicore-chat";
pub const PROFILE: &str = "sap_aicore_chat";
pub const AUTH_MODE: &str = "sap_aicore_service_key";
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub(crate) fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid SAP AI Core service key, deployment or unsupported request.")
        .with_code("invalid_sap_aicore")
}
pub(crate) fn failure(code: &'static str) -> GatewayError {
    let mut error = GatewayError::server_error("SAP AI Core request failed.").with_code(code);
    error.http_status = Some(502);
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "SAP AI Core request failed; no automatic generation replay.".into(),
    };
    error
}
pub(crate) fn stream_error() -> crate::protocol::stream_error::ProtocolStreamError {
    crate::protocol::stream_error::ProtocolStreamError::classified(
        "invalid_sap_aicore_stream: incomplete or invalid Chat stream",
        failure("sap_aicore_response_failed"),
    )
}
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-sap-aicore-chat"))]
mod tests;
#[cfg(all(test, feature = "line-sap-aicore-chat"))]
mod wire_tests;
