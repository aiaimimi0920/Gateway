// ---------------------------------------------------------------------------
// Pipeline stage 5 — finalization
//
// After a successful non-streaming response:
//   - Enqueues a usage report to Redis
//   - Stores the response in the response cache
//
// Streaming responses record metrics inside the TrackedStream callback, so
// finalize is effectively a no-op for SSE.
//
// Finalization errors are logged but not propagated — the response has already
// been (or is being) returned to the client.
// ---------------------------------------------------------------------------

use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use tracing::{debug, warn};

use crate::conversation_archive::{
    archive_user_id, is_conversation_archive_endpoint, persist_conversation_archive,
    PersistConversationArchiveInput,
};
use crate::db;
use crate::protocol::registry::{
    ANTHROPIC_MESSAGES_FAMILY, OPENAI_AUDIO_SPEECH_FAMILY, OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY,
    OPENAI_CHAT_FAMILY, OPENAI_EMBEDDINGS_FAMILY, OPENAI_IMAGES_EDITS_FAMILY,
    OPENAI_IMAGES_GENERATIONS_FAMILY, OPENAI_LEGACY_COMPLETIONS_FAMILY,
    OPENAI_MUSIC_GENERATIONS_FAMILY, OPENAI_REALTIME_FAMILY, OPENAI_RESPONSES_FAMILY,
    OPENAI_VIDEOS_GENERATIONS_FAMILY, SEARCH_API_FAMILY,
};
use crate::provider_failure::classify_provider_failure;
use crate::redis::usage_tracking::{
    parse_upstream_usage, refund_quota, settle_quota_after_usage, TokenUsage, UsageReport,
};
use crate::state::AppState;

use super::runtime_storage::{publish_usage, set_affinity};
use super::PipelineContext;

mod archive;
mod audit_begin;
mod audit_persistence;
mod buffered;
mod credential_model;
mod failure;
mod quota;
mod route_trace;
mod streaming;
mod timestamps;

pub use audit_begin::{begin_request_audit, snapshot_request_audit};
pub use buffered::{run, run_non_json_success};
pub use failure::{run_failure, run_stream_failure};
pub use streaming::run_stream_success;

use archive::{
    build_archive_request_payload_from_ctx, persist_conversation_archive_failure,
    persist_conversation_archive_success, persist_stream_conversation_archive_failure,
    persist_stream_conversation_archive_success,
};
use audit_persistence::{
    finalize_request_audit, finalize_request_audit_failure,
    finalize_request_audit_failure_snapshot, finalize_request_audit_from_snapshot,
    finalize_request_audit_success,
};
#[cfg(test)]
use buffered::extract_usage;
use credential_model::{
    record_credential_model_failure, record_credential_model_failure_from_snapshot,
    record_credential_model_success, record_credential_model_success_from_snapshot,
};
use quota::{
    refund_pre_deducted_quota, refund_pre_deducted_quota_snapshot, settle_pre_deducted_quota,
    settle_pre_deducted_quota_snapshot,
};
use route_trace::{
    build_route_trace_from_ctx, build_route_trace_from_snapshot, endpoint_kind_name,
    extract_response_id, protocol_family_name,
};
#[cfg(test)]
use streaming::apply_stream_completion_semantics;
#[cfg(test)]
use timestamps::format_unix_secs_as_iso;
use timestamps::{format_instant_as_iso, format_now_as_iso};

#[derive(Debug, Clone)]
pub struct RequestAuditFinalizeSnapshot {
    pub cash_credential_id: Option<String>,
    pub cash_charge: Option<Arc<crate::cash_billing::finalization::CashGuard>>,
    pub request_id: String,
    pub request_audit_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub credential_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub platform_access_id: Option<String>,
    pub real_credential_ref: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub route_selection_strategy: Option<String>,
    pub selected_provider_label: Option<String>,
    pub selected_adapter: Option<String>,
    pub selected_protocol_profile: Option<String>,
    pub selected_execution_mode: Option<String>,
    pub selected_upstream_target_protocol_family: Option<String>,
    pub selected_upstream_target_conversation_family: Option<String>,
    pub canonical_conversation_semantics: Option<String>,
    pub selected_upstream_target_tool_family: Option<String>,
    pub tool_strategy: Option<String>,
    pub canonical_tool_choice_semantics: Option<String>,
    pub canonical_completion_semantics: Option<String>,
    pub selected_routing_score: Option<f64>,
    pub selected_health_weight: Option<f64>,
    pub selected_capacity_weight: Option<f64>,
    pub selected_degraded: Option<bool>,
    pub selected_breaker_open: Option<bool>,
    pub selected_degradation_reasons: Vec<String>,
    pub route_attempt_count: u32,
    pub attempted_provider_ids: Vec<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub request_payload: Value,
    pub response_id: Option<String>,
    pub client_has_cache_control: bool,
    pub auto_cache_applied: bool,
    pub pre_deducted_tokens: u64,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub struct FailureFinalizeSnapshot {
    pub request_id: String,
    pub credential_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub started_at: Instant,
    pub pre_deducted_tokens: u64,
    pub request_audit: RequestAuditFinalizeSnapshot,
}

#[cfg(test)]
mod tests;
