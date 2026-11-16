mod accio_model_probe;
#[path = "provider_account_probe.rs"]
mod account_probe;
mod agnes_model_probe;
mod anthropic_model_probe;
mod antigravity_model_probe;
mod bedrock_model_probe;
mod chatgpt_web_model_probe;
mod codex_model_probe;
mod cohere_model_probe;
mod commandcode_model_probe;
mod console_model_probe;
mod console_probe;
mod console_probe_request;
#[cfg(feature = "copilot-sdk-runtime")]
pub use gateway_copilot_runtime as copilot;
mod dim_model_probe;
mod gemini_model_probe;
mod grok_web_model_probe;
#[cfg(all(test, feature = "line-grok-web-reverse-api"))]
mod grok_web_model_probe_tests;
mod http_probe;
mod kiro_api_key_model_probe;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod kiro_api_key_probe_tests;
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod kiro_oauth_runtime_probe_tests;
mod lobsterai_model_probe;
mod lumalabs_media_probe;
mod management;
mod media_model_probe;
mod media_probe_request;
mod minimax_model_probe;
mod model_probe_audit;
mod model_probe_health;
mod model_probe_recording;
mod nous_model_probe;
mod openai_model_probe_response;
mod perplexity_web_model_probe;
#[cfg(all(test, feature = "line-perplexity-web-reverse-api"))]
mod perplexity_web_model_probe_tests;
mod probe_lock;
mod qwen_model_probe;
#[cfg(all(test, feature = "line-qwen-official-api"))]
mod qwen_model_probe_tests;
mod qwen_web_model_probe;
#[cfg(all(test, feature = "line-qwen-web-reverse"))]
mod qwen_web_probe_tests;
mod recording;
mod sap_aicore_model_probe;
mod search_model_probe;
mod sensenova_model_probe;
mod suno_media_probe;
pub(crate) mod test_execution;
pub mod test_measurement;
pub mod test_quota;
pub mod test_results;
pub(crate) mod test_store;
#[cfg(test)]
mod tests;
mod udio_media_probe;
#[cfg(all(test, feature = "line-google-agent-platform-official"))]
mod vertex_express_probe_tests;

pub use account_probe::probe_provider_account_payload;
pub(crate) use console_model_probe::probe_console_target;
pub(crate) use console_model_probe::probe_console_target_with_request;
pub use console_probe::{probe_provider_payload_for_console, provider_payload_probe_point};
pub(crate) use console_probe_request::ConsoleProbeRequest;
pub(crate) use console_probe_request::ConsoleTestScope;
pub use management::{
    probe_provider_account_for_management, sweep_cooling_provider_accounts,
    sweep_cooling_provider_accounts_best_effort,
};
pub use recording::{
    clear_provider_credential_runtime_keys, read_provider_breaker_open, read_runtime_breaker_open,
    record_provider_candidate_failure, record_provider_candidate_success,
    record_provider_credential_probe_report, record_provider_failure, record_provider_success,
};

use rquest::Method;
use serde::{Deserialize, Serialize};

use crate::db;
use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::protocol::freebuff;
use crate::provider_quota::GatewayProviderQuotaView;
use crate::state::AppState;
use http_probe::{
    build_probe_http_client, probe_known_http_payload, send_probe_request, ProbeExpectation,
    ProbePolicy,
};

#[derive(Debug, Clone)]
pub struct ProviderProbeOutcome {
    pub ok: bool,
    pub provider_account: db::GatewayProviderAccountView,
    pub error_message: Option<String>,
    pub provider_quota: Option<GatewayProviderQuotaView>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderPayloadProbeStatus {
    Passed,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct ProviderPayloadProbeReport {
    pub health_recovery:
        Option<crate::db::provider_credential_model_states::recovery::ModelHealthRecovery>,
    pub status: ProviderPayloadProbeStatus,
    pub message: String,
    /// Redacted generated text for deterministic assessment; health probes have none.
    pub answer: Option<String>,
}

fn provider_error_message_for_persistence(message: &str) -> String {
    sanitize_provider_error_message(message)
}
