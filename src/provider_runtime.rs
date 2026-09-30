#[path = "provider_account_probe.rs"]
mod account_probe;
mod codex_model_probe;
mod console_model_probe;
mod console_probe;
mod http_probe;
mod management;
mod model_probe_recording;
mod probe_lock;
mod recording;
#[cfg(test)]
mod tests;

pub use account_probe::probe_provider_account_payload;
pub(crate) use console_model_probe::probe_console_target;
pub use console_probe::{probe_provider_payload_for_console, provider_payload_probe_point};
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
use serde::Serialize;

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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderPayloadProbeStatus {
    Passed,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct ProviderPayloadProbeReport {
    pub status: ProviderPayloadProbeStatus,
    pub message: String,
}

fn provider_error_message_for_persistence(message: &str) -> String {
    sanitize_provider_error_message(message)
}
