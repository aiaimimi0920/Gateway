// ---------------------------------------------------------------------------
// pipeline/mod.rs — request pipeline
//
// Stages:
//   1. stage_auth     — authenticate and populate session
//   2. stage_filter   — content filter + quota check
//   3. stage_route    — build candidate queue
//   4. stage_rate_limit — enforce route-policy request admission
//   5. stage_send     — dispatch to upstream with retry/fallback
//   6. stage_finalize — usage reporting and response caching
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use uuid::Uuid;

use crate::auth::session::AuthenticatedSession;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, TokenUsage};
use crate::routing::candidate::RouteCandidate;
use crate::state::AppState;
use crate::upstream::stream::TrackedStream;

pub mod request_budget;
mod route_health_cache;
mod runtime_storage;
pub mod stage_auth;
pub mod stage_filter;
pub mod stage_finalize;
pub mod stage_rate_limit;
pub mod stage_route;
mod stage_route_health;
pub mod stage_send;

// ---------------------------------------------------------------------------
// CredentialSource
// ---------------------------------------------------------------------------

/// Determines which credential pool the gateway uses for this request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    /// Use the shared platform credential pool (YAML config + Redis shared pool).
    /// This is the default for "unlimited calls" business model.
    Platform,
    /// Use the requesting user's hosted credentials (Redis per-user pool).
    /// Used for "unlimited refill" and "self-hosted key" business models.
    Hosted,
}

impl Default for CredentialSource {
    fn default() -> Self {
        Self::Platform
    }
}

// ---------------------------------------------------------------------------
// PipelineContext
// ---------------------------------------------------------------------------

/// Mutable state threaded through all pipeline stages for a single request.
pub struct PipelineContext {
    /// Unique identifier for this request (used in logs and reports).
    pub req_id: Uuid,
    /// The parsed, normalized canonical request.
    pub canonical_req: CanonicalRelayRequest,
    /// Convenience copy of `canonical_req.stream`.
    pub stream: bool,
    /// Wall-clock timestamp when the pipeline started.
    pub started_at: Instant,
    /// Single send/deadline owner shared by all candidates and recoveries.
    pub request_budget: request_budget::RequestBudget,
    /// Exact IDs explicitly named by trusted route/key/projection owners, not auto discovery.
    pub explicit_fallback_provider_ids: Vec<String>,
    /// Bearer token extracted from the HTTP Authorization header, if present.
    pub bearer_token: Option<String>,

    // ── Populated by stages ───────────────────────────────────────────────
    /// Authenticated session; populated by stage_auth.
    pub session: Option<AuthenticatedSession>,
    /// Ordered list of provider candidates; populated by stage_route.
    pub candidates: Vec<RouteCandidate>,
    /// Unified projected access rows aligned with `candidates` order when
    /// access-catalog routing is active.
    pub projected_access_candidates: Vec<crate::db::ProjectedPlatformAccessRow>,
    /// Opaque reference to the credential used (for usage reporting).
    pub credential_ref: Option<String>,
    /// Credential/account key actually used for quota pre-deduct.
    pub quota_credential_id: Option<String>,
    /// Tokens pre-deducted during stage_filter.
    pub quota_pre_deducted_tokens: u64,
    /// Unified access key that initiated the request.
    pub requesting_access_key_id: Option<String>,
    /// Unified source access key ultimately billed for the request.
    pub source_access_key_id: Option<String>,
    /// Selected platform access catalog row.
    pub selected_platform_access_id: Option<String>,
    /// Real provider credential ref chosen for the upstream call.
    pub selected_real_credential_ref: Option<String>,
    /// Active route policy resolved for this request, when DB-owned routing is used.
    pub route_policy_id: Option<String>,
    /// Request-audit row ID when audit persistence is enabled for this request.
    pub request_audit_id: Option<String>,
    /// Resolved route policy config for runtime decisions such as circuit breakers.
    pub route_policy_config: Option<crate::db::GatewayRoutePolicyConfig>,
    /// Queue selection strategy resolved from the active route policy.
    pub route_selection_strategy: Option<String>,
    /// Number of provider attempts performed in stage_send.
    pub route_attempt_count: Arc<AtomicU32>,
    /// Unique provider accounts that received at least one admitted outbound attempt,
    /// in first-contact order. Retries increment the count but do not duplicate IDs.
    pub attempted_provider_ids: Arc<Mutex<Vec<String>>>,

    // ── Provider selection info (populated by stage_send) ─────────────────
    /// Provider account ID of the winning candidate.
    pub selected_provider_id: Option<String>,
    /// Provider credential ID of the winning candidate, when route selection
    /// happened at single-credential granularity.
    pub selected_provider_credential_id: Option<String>,
    /// Human-readable label of the winning candidate.
    pub selected_provider_label: Option<String>,
    /// Adapter identifier of the winning candidate (e.g. "openai_compatible").
    pub selected_adapter: Option<String>,
    /// Protocol profile of the winning candidate.
    pub selected_protocol_profile: Option<String>,
    /// Resolved model name sent to the upstream.
    pub resolved_model: Option<String>,
    /// Model alias that resolved to the winning candidate, when present.
    pub selected_model_alias: Option<String>,
    /// Resolved execution mode of the winning candidate.
    pub selected_execution_mode: Option<String>,
    /// Canonical outbound protocol family selected for the upstream request.
    pub selected_upstream_target_protocol_family: Option<String>,
    /// Canonical conversation family selected for the upstream request.
    pub selected_upstream_target_conversation_family: Option<String>,
    /// Canonical conversation semantics carried by this request.
    pub canonical_conversation_semantics: Option<String>,
    /// Canonical tool family selected for the upstream request.
    pub selected_upstream_target_tool_family: Option<String>,
    /// Whether tools stayed native or were downgraded to XML fallback.
    pub tool_strategy: Option<String>,
    /// Canonical tool-choice semantics for this request.
    pub canonical_tool_choice_semantics: Option<String>,
    /// Canonical completion semantics observed from the winning response.
    pub canonical_completion_semantics: Option<String>,
    /// Composite routing score of the winning candidate.
    pub selected_routing_score: Option<f64>,
    /// Health contribution of the winning candidate routing score.
    pub selected_health_weight: Option<f64>,
    /// Capacity contribution of the winning candidate routing score.
    pub selected_capacity_weight: Option<f64>,
    /// Whether the winning candidate was degraded when selected.
    pub selected_degraded: Option<bool>,
    /// Whether the winning candidate had an open breaker when scored.
    pub selected_breaker_open: Option<bool>,
    /// Degradation reasons recorded for the winning candidate.
    pub selected_degradation_reasons: Vec<String>,
    /// Whether the outgoing Anthropic request already contained client-supplied
    /// prompt-cache markers before Rust packing touched it.
    pub client_has_cache_control: bool,
    /// Whether Rust auto-applied Anthropic prompt-cache markers for the
    /// outgoing request.
    pub auto_cache_applied: bool,
    /// Canonical usage captured from the upstream before the response is
    /// repacked into the client-facing protocol shape.
    pub observed_usage: Option<TokenUsage>,

    // ── Tool injection state ────────────────────────────────────────────
    /// Whether XML tool definitions were injected into the request because
    /// the target model lacks native tool-calling support.  Used by the
    /// streaming path to decide whether to wrap the byte stream with a
    /// tool-call detector.
    pub tools_were_injected: bool,

    // ── Credential routing (populated by stage_auth) ───────────────────
    /// Which credential pool to use: shared platform pool or user's hosted pool.
    /// Populated by stage_auth from `X-Neuro-Cred-Source` header.
    pub credential_source: CredentialSource,

    /// User ID injected by the platform layer via `X-Neuro-User` header.
    /// Used for user-scoped credential lookup in hosted mode.
    pub neuro_user_id: Option<String>,

    /// Trusted account-group selector consumed only by Gateway routing.
    /// This must never be copied into `request_headers` or sent upstream.
    pub account_group_id: Option<String>,

    // ── Request metadata (populated by route handlers) ────────────────────
    /// Query parameters extracted from the request URI.
    pub query_params: HashMap<String, String>,
    /// Selected request headers forwarded to the auth stage and upstream.
    pub request_headers: HashMap<String, String>,
}

impl PipelineContext {
    /// Create a new context for a request.
    pub fn new(canonical_req: CanonicalRelayRequest, bearer_token: Option<String>) -> Self {
        let stream = canonical_req.stream;
        let request_budget = request_budget::RequestBudget::for_request(&canonical_req);
        Self {
            req_id: Uuid::new_v4(),
            canonical_req,
            stream,
            started_at: Instant::now(),
            request_budget,
            explicit_fallback_provider_ids: Vec::new(),
            bearer_token,
            session: None,
            candidates: vec![],
            projected_access_candidates: vec![],
            credential_ref: None,
            quota_credential_id: None,
            quota_pre_deducted_tokens: 0,
            requesting_access_key_id: None,
            source_access_key_id: None,
            selected_platform_access_id: None,
            selected_real_credential_ref: None,
            route_policy_id: None,
            request_audit_id: None,
            route_policy_config: None,
            route_selection_strategy: None,
            route_attempt_count: Arc::new(AtomicU32::new(0)),
            attempted_provider_ids: Arc::new(Mutex::new(Vec::new())),
            selected_provider_id: None,
            selected_provider_credential_id: None,
            selected_provider_label: None,
            selected_adapter: None,
            selected_protocol_profile: None,
            resolved_model: None,
            selected_model_alias: None,
            selected_execution_mode: None,
            selected_upstream_target_protocol_family: None,
            selected_upstream_target_conversation_family: None,
            canonical_conversation_semantics: None,
            selected_upstream_target_tool_family: None,
            tool_strategy: None,
            canonical_tool_choice_semantics: None,
            canonical_completion_semantics: None,
            selected_routing_score: None,
            selected_health_weight: None,
            selected_capacity_weight: None,
            selected_degraded: None,
            selected_breaker_open: None,
            selected_degradation_reasons: Vec::new(),
            client_has_cache_control: false,
            auto_cache_applied: false,
            observed_usage: None,
            tools_were_injected: false,
            credential_source: CredentialSource::default(),
            neuro_user_id: None,
            account_group_id: None,
            query_params: HashMap::new(),
            request_headers: HashMap::new(),
        }
    }

    pub fn route_attempt_count(&self) -> u32 {
        self.route_attempt_count.load(Ordering::Relaxed)
    }

    pub fn note_provider_attempt(&self, provider_account_id: &str) {
        self.route_attempt_count.fetch_add(1, Ordering::Relaxed);
        let mut attempted = self.attempted_provider_ids.lock();
        if !attempted.iter().any(|value| value == provider_account_id) {
            attempted.push(provider_account_id.to_string());
        }
    }

    pub fn attempted_provider_ids(&self) -> Vec<String> {
        self.attempted_provider_ids.lock().clone()
    }

    /// The provider a request audit belongs to.
    ///
    /// A request that reached an upstream names the candidate that answered it.
    /// A request whose every attempt failed has no winner, yet the failure still
    /// belongs to the provider that produced it — the last one tried. Leaving
    /// those rows unattributed would make the console's per-provider success rate
    /// count successes only, so it could never fall below 100%.
    pub fn audited_provider_id(&self) -> Option<String> {
        if let Some(selected) = self.selected_provider_id.clone() {
            return Some(selected);
        }
        self.attempted_provider_ids.lock().last().cloned()
    }
}

// ---------------------------------------------------------------------------
// PipelineOutput
// ---------------------------------------------------------------------------

/// The result produced by the pipeline after stage_send.
pub enum PipelineOutput {
    /// A complete JSON response body (non-streaming requests).
    Json(serde_json::Value),
    /// A live byte stream wrapping the upstream SSE response.
    Sse(TrackedStream<crate::protocol::stream_error::StreamError<rquest::Error>>),
    /// A non-JSON binary passthrough response.
    Binary(BinaryPipelineResponse),
}

pub struct BinaryPipelineResponse {
    pub body: bytes::Bytes,
    pub content_type: Option<String>,
    pub extra_headers: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// run_pipeline
// ---------------------------------------------------------------------------

/// Execute all six pipeline stages in order.
///
/// The first five stages can return errors that abort the pipeline. Stage 6
/// (finalize) is best-effort and never aborts.
pub fn run_pipeline(
    ctx: PipelineContext,
    state: &Arc<AppState>,
) -> impl std::future::Future<Output = Result<PipelineOutput, GatewayError>> + Send + '_ {
    // All HTTP callers retain only a pointer-sized future, not the full pipeline.
    // Poll in the same task: cancellation still drops the original stage owners.
    Box::pin(run_pipeline_inner(ctx, state))
}

async fn run_pipeline_inner(
    mut ctx: PipelineContext,
    state: &Arc<AppState>,
) -> Result<PipelineOutput, GatewayError> {
    // Stage 1 — authenticate
    if let Err(error) = stage_auth::run(&mut ctx, state).await {
        stage_finalize::run_failure(&error, &ctx, state).await;
        return Err(error);
    }
    stage_finalize::begin_request_audit(&mut ctx, state).await;

    // Stage 2 — content filter
    if let Err(error) = stage_filter::run(&mut ctx, state).await {
        stage_finalize::run_failure(&error, &ctx, state).await;
        return Err(error);
    }

    // Stage 3 — route resolution
    if let Err(error) = stage_route::run(&mut ctx, state).await {
        stage_finalize::run_failure(&error, &ctx, state).await;
        return Err(error);
    }

    // Stage 4 — route-policy request admission
    if let Err(error) = stage_rate_limit::run(&mut ctx, state).await {
        stage_finalize::run_failure(&error, &ctx, state).await;
        return Err(error);
    }

    // Stage 5 — upstream dispatch
    // Keep the large dispatch future out of its enclosing async state machines.
    // Await it in this task so cancellation still drops the same dispatch owners.
    let output = match Box::pin(stage_send::run(&mut ctx, state)).await {
        Ok(output) => output,
        Err(error) => {
            stage_finalize::run_failure(&error, &ctx, state).await;
            return Err(error);
        }
    };

    // Stage 6 — finalize (best-effort; does not block response)
    // For JSON responses, clone the body so we don't hold a reference to the
    // non-Sync PipelineOutput across the finalize await point.
    // Streaming finalization happens inside the TrackedStream callback.
    match &output {
        PipelineOutput::Json(v) => {
            let json_clone = v.clone();
            stage_finalize::run(Some(&json_clone), &ctx, state).await;
        }
        PipelineOutput::Sse(_) => {
            stage_finalize::run(None, &ctx, state).await;
        }
        PipelineOutput::Binary(_) => {
            stage_finalize::run_non_json_success(&ctx, state).await;
        }
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use std::collections::HashMap;

    fn make_canonical_req() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn context_has_unique_req_id() {
        let ctx1 = PipelineContext::new(make_canonical_req(), None);
        let ctx2 = PipelineContext::new(make_canonical_req(), None);
        assert_ne!(ctx1.req_id, ctx2.req_id);
    }

    #[test]
    fn context_stream_flag_matches_request() {
        let mut req = make_canonical_req();
        req.stream = true;
        let ctx = PipelineContext::new(req, None);
        assert!(ctx.stream);
    }

    #[test]
    fn context_default_fields_are_none() {
        let ctx = PipelineContext::new(make_canonical_req(), None);
        assert!(ctx.session.is_none());
        assert!(ctx.candidates.is_empty());
        assert!(ctx.credential_ref.is_none());
        assert!(ctx.quota_credential_id.is_none());
        assert_eq!(ctx.quota_pre_deducted_tokens, 0);
        assert!(ctx.route_policy_id.is_none());
        assert!(ctx.route_policy_config.is_none());
        assert!(ctx.route_selection_strategy.is_none());
        assert!(ctx.bearer_token.is_none());
        // New provider-selection fields default to None.
        assert!(ctx.selected_provider_id.is_none());
        assert!(ctx.selected_provider_label.is_none());
        assert!(ctx.selected_adapter.is_none());
        assert!(ctx.selected_protocol_profile.is_none());
        assert!(ctx.resolved_model.is_none());
        assert!(ctx.selected_upstream_target_protocol_family.is_none());
        assert!(ctx.selected_upstream_target_conversation_family.is_none());
        assert!(ctx.canonical_conversation_semantics.is_none());
        assert!(ctx.selected_upstream_target_tool_family.is_none());
        assert!(ctx.tool_strategy.is_none());
        assert!(ctx.canonical_tool_choice_semantics.is_none());
        assert!(ctx.canonical_completion_semantics.is_none());
        assert!(ctx.selected_routing_score.is_none());
        assert!(ctx.selected_health_weight.is_none());
        assert!(ctx.selected_capacity_weight.is_none());
        assert!(ctx.selected_degraded.is_none());
        assert!(ctx.selected_breaker_open.is_none());
        assert!(ctx.selected_degradation_reasons.is_empty());
        // Tool injection flag defaults to false.
        assert!(!ctx.tools_were_injected);
    }

    #[test]
    fn context_bearer_token_stored() {
        let ctx = PipelineContext::new(make_canonical_req(), Some("my-token".to_string()));
        assert_eq!(ctx.bearer_token.as_deref(), Some("my-token"));
    }

    #[test]
    fn context_new_fields_initialize_to_empty() {
        let ctx = PipelineContext::new(make_canonical_req(), None);
        assert!(ctx.query_params.is_empty());
        assert!(ctx.request_headers.is_empty());
    }

    #[test]
    fn context_provider_fields_can_be_set() {
        let mut ctx = PipelineContext::new(make_canonical_req(), None);
        ctx.selected_provider_id = Some("prov-1".to_string());
        ctx.selected_provider_label = Some("My Provider".to_string());
        ctx.selected_adapter = Some("openai_compatible".to_string());
        ctx.selected_protocol_profile = Some("openai".to_string());
        ctx.resolved_model = Some("gpt-4o".to_string());
        ctx.selected_upstream_target_protocol_family = Some("openai_chat".to_string());
        ctx.selected_upstream_target_conversation_family = Some("openai_chat".to_string());
        ctx.selected_upstream_target_tool_family = Some("openai_chat".to_string());
        ctx.canonical_tool_choice_semantics = Some("required".to_string());
        ctx.selected_routing_score = Some(0.91);
        assert_eq!(ctx.selected_provider_id.as_deref(), Some("prov-1"));
        assert_eq!(ctx.selected_provider_label.as_deref(), Some("My Provider"));
        assert_eq!(ctx.selected_adapter.as_deref(), Some("openai_compatible"));
        assert_eq!(ctx.resolved_model.as_deref(), Some("gpt-4o"));
        assert_eq!(
            ctx.selected_upstream_target_protocol_family.as_deref(),
            Some("openai_chat")
        );
        assert_eq!(
            ctx.selected_upstream_target_conversation_family.as_deref(),
            Some("openai_chat")
        );
        assert_eq!(
            ctx.selected_upstream_target_tool_family.as_deref(),
            Some("openai_chat")
        );
        assert_eq!(
            ctx.canonical_tool_choice_semantics.as_deref(),
            Some("required")
        );
        assert_eq!(ctx.selected_routing_score, Some(0.91));
    }

    #[test]
    fn context_request_headers_can_be_populated() {
        let mut ctx = PipelineContext::new(make_canonical_req(), None);
        ctx.request_headers
            .insert("x-api-key".to_string(), "sk-test".to_string());
        ctx.query_params
            .insert("beta".to_string(), "true".to_string());
        assert_eq!(
            ctx.request_headers.get("x-api-key").map(String::as_str),
            Some("sk-test")
        );
        assert_eq!(
            ctx.query_params.get("beta").map(String::as_str),
            Some("true")
        );
    }
}
