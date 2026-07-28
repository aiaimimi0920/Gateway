// ---------------------------------------------------------------------------
// Pipeline stage 3 — route candidate resolution (dual resolution)
//
// Resolves the ordered list of upstream provider candidates for this request.
// Resolution order:
//   1. Redis credentials (user-owned via X-Credential-Ref, then project pool)
//   2. YAML static config (fallback)
//   3. Error if both sources yield zero candidates
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{debug, warn};

use crate::db;
use crate::error::GatewayError;
use crate::provider_quota;
use crate::provider_runtime;
use crate::redis::credential_cache::{
    get_credential, get_credential_affinity, lookup_credentials_by_model, CredentialKind,
};
use crate::routing::candidate::RouteCandidate;
use crate::routing::config::RouteAccountGroupConstraint;
use crate::routing::credential_routing::credential_to_candidate;
use crate::routing::protocol_resolution::{
    finalize_candidate_protocol_family, route_policy_family_matches_candidate,
};
use crate::routing::queue::build_candidate_queue;
use crate::routing::queue::HealthSnapshot;
use crate::state::AppState;

use super::{CredentialSource, PipelineContext};

/// Resolve the candidate queue for this request using dual resolution:
/// Redis credentials first, then YAML static config as fallback.
///
/// **User isolation rules:**
/// - `X-Credential-Ref`: validated against the requesting user's identity
/// - `UserOwned` / `AccountCredential`: only matched to their owner
/// - `PlatformUnlimited` / `PlatformLimited`: shared across all project users
pub async fn run(ctx: &mut PipelineContext, state: &Arc<AppState>) -> Result<(), GatewayError> {
    let model = ctx.canonical_req.requested_model.as_deref();
    let requested_account_group_id = ctx.account_group_id.clone();
    let account_group_constraint = state
        .route_config
        .account_group_constraint(requested_account_group_id.as_deref())
        .map_err(map_account_group_selection_error)?;
    let credential_ref = ctx
        .credential_ref
        .clone()
        .or_else(|| ctx.request_headers.get("x-credential-ref").cloned());
    let explicit_credential_requested = credential_ref.is_some();

    // Extract identity from auth session for credential isolation.
    let project_id = ctx
        .session
        .as_ref()
        .map(|s| s.project_id.as_str())
        .unwrap_or("default");
    // Use platform-injected user ID (X-Neuro-User) if available,
    // otherwise fall back to the auth session's user_id.
    let user_id = ctx
        .neuro_user_id
        .as_deref()
        .or_else(|| ctx.session.as_ref().and_then(|s| s.user_id.as_deref()));

    if let Some(access_key_id) = ctx
        .session
        .as_ref()
        .and_then(|session| session.access_key_id.as_deref())
    {
        let Some(pg_pool) = &state.pg_pool else {
            return Err(GatewayError::service_unavailable(
                "统一 access key 路由依赖 PostgreSQL",
            ));
        };
        let requested_model = model
            .ok_or_else(|| GatewayError::bad_request("当前请求缺少模型标识，无法匹配访问表"))?;
        let estimated =
            crate::redis::usage_tracking::estimate_token_count(&ctx.canonical_req.messages_text())
                .max(1);
        let route_context = db::resolve_access_key_route_context(
            pg_pool,
            &state.redis_pool,
            access_key_id,
            requested_model,
            ctx.canonical_req.endpoint_kind,
            estimated,
            ctx.canonical_req.explicit_session_key.as_deref(),
        )
        .await?;
        let (route_candidates, projected_candidates) = filter_candidate_pairs_by_account_group(
            &account_group_constraint,
            route_context.route_candidates,
            route_context.candidates,
        );
        if route_candidates.is_empty() {
            if let Some(error) = account_group_candidates_unavailable_error(
                &account_group_constraint,
                Some(requested_model),
            ) {
                return Err(error);
            }
            return Err(GatewayError::quota_exceeded("当前 key 没有可用的访问候选")
                .with_code("access_candidates_exhausted"));
        }
        if ctx
            .session
            .as_ref()
            .and_then(|s| s.access_key_kind.as_deref())
            == Some("auto_route")
        {
            if let Some(selected) = projected_candidates.first() {
                let decision = db::pre_deduct_access_key_balance(
                    pg_pool,
                    &state.redis_pool,
                    &selected.source_access_key_id,
                    estimated,
                )
                .await?;
                if !decision.allowed {
                    return Err(
                        GatewayError::quota_exceeded("当前自动路由 key 的候选额度不足").with_code(
                            decision
                                .reason
                                .unwrap_or_else(|| "balance_not_allowed".to_string()),
                        ),
                    );
                }
                ctx.quota_credential_id = Some(selected.source_access_key_id.clone());
                ctx.quota_pre_deducted_tokens = decision.pre_deduct_amount;
            }
        }
        let finalized = finalize_candidate_pairs_for_request(
            &ctx.canonical_req,
            route_candidates,
            Some(projected_candidates),
        );
        ctx.projected_access_candidates = finalized.1.unwrap_or_default();
        ctx.candidates = finalized.0;
        if ctx.candidates.is_empty() {
            let projected_candidate_debug = ctx
                .projected_access_candidates
                .iter()
                .map(|row| {
                    format!(
                        "platformAccessId={} providerAccountId={} modelCode={} endpointKind={} upstreamModel={}",
                        row.platform_access_id,
                        row.provider_account_id,
                        row.model_code,
                        row.endpoint_kind,
                        row.upstream_model.as_deref().unwrap_or("<none>")
                    )
                })
                .collect::<Vec<_>>();
            warn!(
                req_id = %ctx.req_id,
                model = requested_model,
                endpoint_kind = ?ctx.canonical_req.endpoint_kind,
                projected_candidate_count = projected_candidate_debug.len(),
                projected_candidates = ?projected_candidate_debug,
                "all projected access candidates were filtered out after protocol-family finalization"
            );
            return Err(GatewayError::quota_exceeded("当前 key 没有可用的访问候选")
                .with_code("access_candidates_exhausted"));
        }
        ctx.route_selection_strategy = Some("access_catalog".to_string());
        return Ok(());
    }

    // ── Phase 1: Try Redis credential resolution ─────────────────────────
    let mut redis_candidates = Vec::new();

    if let Some(ref cred_ref) = credential_ref {
        // User-specific credential via X-Credential-Ref.
        // SECURITY: verify the credential belongs to this user or is a
        // platform credential in the same project.
        match get_credential(&state.redis_pool, cred_ref).await {
            Ok(Some(cred)) => {
                let allowed = match cred.kind {
                    // Personal credentials: must belong to the requesting user
                    CredentialKind::UserOwned | CredentialKind::AccountCredential => {
                        user_id.map_or(false, |uid| cred.user_id == uid)
                    }
                    // Platform credentials: same project is enough
                    CredentialKind::PlatformUnlimited | CredentialKind::PlatformLimited => {
                        cred.project_id == project_id
                    }
                };

                if !allowed {
                    warn!(
                        req_id = %ctx.req_id,
                        credential_ref = cred_ref,
                        cred_user = %cred.user_id,
                        req_user = ?user_id,
                        "X-Credential-Ref ownership mismatch; rejecting"
                    );
                    return Err(GatewayError::unauthorized(
                        "Credential does not belong to the requesting user",
                    ));
                }

                let m = model.unwrap_or("");
                if !m.is_empty() && !cred.supports_model(m) {
                    return Err(GatewayError::bad_request(format!(
                        "Requested credential '{}' does not support model '{}'",
                        cred_ref, m
                    )));
                }

                match credential_to_candidate(&cred, m) {
                    Some(candidate) => redis_candidates.push(candidate),
                    None => {
                        return Err(GatewayError::bad_request(format!(
                            "Requested credential '{}' is missing required runtime fields",
                            cred_ref
                        )));
                    }
                }
            }
            Ok(None) => {
                warn!(
                    req_id = %ctx.req_id,
                    credential_ref = cred_ref,
                    "X-Credential-Ref credential not found"
                );
                return Err(GatewayError::bad_request(format!(
                    "Requested credential '{}' was not found",
                    cred_ref
                )));
            }
            Err(e) => {
                warn!(
                    req_id = %ctx.req_id,
                    credential_ref = cred_ref,
                    error = %e,
                    "X-Credential-Ref lookup failed"
                );
                return Err(
                    GatewayError::server_error("Failed to resolve requested credential")
                        .with_code("credential_lookup_failed"),
                );
            }
        }
    }

    // If no explicit credential resolved, query the credential pool.
    // Two-tier cache: in-memory first (O(1)), then Redis model index.
    if redis_candidates.is_empty() {
        if let Some(m) = model {
            // Tier 1: In-memory cache (DashMap, per-process, 30s TTL)
            if let Some(cached) = state.credential_cache.get(project_id, m) {
                debug!(
                    req_id = %ctx.req_id,
                    model = m,
                    count = cached.len(),
                    "credential cache HIT (in-memory)"
                );
                for cred in &cached {
                    if let Some(candidate) = credential_to_candidate(cred, m) {
                        redis_candidates.push(candidate);
                    }
                }
            }

            // Tier 2: Cache miss -> Redis model index -> populate cache
            if redis_candidates.is_empty() {
                match lookup_credentials_by_model(&state.redis_pool, m, user_id).await {
                    Ok(creds) => {
                        debug!(
                            req_id = %ctx.req_id,
                            model = m,
                            count = creds.len(),
                            "credential cache MISS -> Redis model index"
                        );
                        // Populate the in-memory cache for next time
                        state.credential_cache.put(project_id, m, creds.clone());
                        for cred in &creds {
                            if let Some(candidate) = credential_to_candidate(cred, m) {
                                redis_candidates.push(candidate);
                            }
                        }
                    }
                    Err(e) => {
                        debug!(
                            req_id = %ctx.req_id,
                            error = %e,
                            "Redis credential lookup failed; falling back to YAML"
                        );
                    }
                }
            }
        }
    }

    // ── Phase 2: Fall back to static YAML config ─────────────────────────
    //
    // In "hosted" mode, the user's own credentials are authoritative — do NOT
    // fall back to the shared platform pool. This ensures user-hosted keys
    // (unlimited refill / self-hosted) are never mixed with platform credentials.
    let mut all_candidates = redis_candidates;
    let mut selection_strategy = "priority_weighted".to_string();

    if !explicit_credential_requested && ctx.credential_source != CredentialSource::Hosted {
        if let Some(pg_pool) = &state.pg_pool {
            provider_runtime::sweep_cooling_provider_accounts_best_effort(state, 10).await;
            let route_context = crate::db::resolve_route_candidates(
                pg_pool,
                project_id,
                model,
                ctx.canonical_req.endpoint_kind,
            )
            .await?;
            ctx.route_policy_id = Some(route_context.route_policy_id);
            ctx.route_policy_config = Some(route_context.route_policy_config.clone());
            selection_strategy = route_context.selection_strategy;
            all_candidates.extend(route_context.candidates);
        } else {
            let yaml_candidates = state
                .route_config
                .resolve_candidates_for_account_group(model, requested_account_group_id.as_deref())
                .map_err(map_account_group_selection_error)?;
            all_candidates.extend(yaml_candidates);
        }
    }

    // Redis and PostgreSQL candidates are already concrete accounts. Apply
    // the same validated group constraint used by access-catalog and YAML
    // routing before affinity or queue ordering can select an outside account.
    all_candidates = account_group_constraint.filter_candidates(all_candidates);

    // ── Credential Affinity: boost sticky credential to front ────────────
    //
    // Upstream providers (especially Anthropic) cache prompt tokens per API
    // key. Switching credentials mid-conversation loses the cache benefit.
    // We use a two-level affinity scope:
    //   1. Session-level: explicit_session_key (conversation stickiness)
    //   2. User-level: user_id + model (fallback stickiness)
    //
    // If an affinity credential is found in the candidate list, promote it
    // to position 0. After the request succeeds (in stage_finalize), the
    // winning credential is recorded as the new affinity.
    if let Some(m) = model {
        if all_candidates.len() > 1 {
            let affinity_scope = ctx
                .canonical_req
                .explicit_session_key
                .as_deref()
                .map(|sk| format!("session:{}", sk))
                .or_else(|| user_id.map(|uid| format!("user:{}", uid)));

            if let Some(ref scope) = affinity_scope {
                match get_credential_affinity(&state.redis_pool, scope, m).await {
                    Ok(Some(affinity_id)) => {
                        // Find the affinity credential in the candidate list
                        if let Some(pos) = all_candidates
                            .iter()
                            .position(|c| c.provider_account_id == affinity_id)
                        {
                            if pos > 0 {
                                let sticky = all_candidates.remove(pos);
                                all_candidates.insert(0, sticky);
                                debug!(
                                    req_id = %ctx.req_id,
                                    affinity_id = %affinity_id,
                                    "credential affinity: boosted to front"
                                );
                            }
                        }
                        // If the affinity credential isn't in the list (maybe
                        // disabled/expired), we just proceed with normal ordering.
                    }
                    _ => {} // No affinity or Redis error — proceed normally
                }
            }
        }
    }

    debug!(
        req_id = %ctx.req_id,
        model = ?model,
        candidate_count = all_candidates.len(),
        selection_strategy = %selection_strategy,
        "building candidate queue"
    );

    all_candidates = finalize_candidate_pairs_for_request(
        &ctx.canonical_req,
        all_candidates,
        None::<Vec<crate::db::ProjectedPlatformAccessRow>>,
    )
    .0;
    all_candidates =
        filter_candidates_by_route_policy_family(all_candidates, ctx.route_policy_config.as_ref());
    if let Some(pg_pool) = &state.pg_pool {
        if let Err(error) = crate::db::apply_provider_credential_model_states_to_candidates(
            pg_pool,
            &mut all_candidates,
        )
        .await
        {
            warn!(
                req_id = %ctx.req_id,
                error = %error,
                "failed to apply provider credential-model health states"
            );
        }
    }

    if all_candidates.is_empty() {
        if let Some(error) =
            account_group_candidates_unavailable_error(&account_group_constraint, model)
        {
            return Err(error);
        }
        let model_str = model.unwrap_or("<none>");
        warn!(
            req_id = %ctx.req_id,
            model = model_str,
            "no route candidates found"
        );
        return Err(GatewayError::bad_request(format!(
            "No providers configured for model '{}'",
            model_str
        )));
    }

    // Build an ordered queue using the priority_weighted strategy.
    let concurrency_snapshots = state.concurrency_registry.snapshot_all();
    let mut health_snapshots = HashMap::new();
    for candidate in &all_candidates {
        let active_concurrency = concurrency_snapshots
            .get(&candidate.provider_account_id)
            .map(|snapshot| snapshot.active_count)
            .unwrap_or(0);
        let balance_status = if candidate.provider_account_id.starts_with("cred:") {
            None
        } else {
            provider_quota::get_or_refresh_runtime_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                &candidate.provider_account_id,
                candidate.provider_credential_id.as_deref(),
                &candidate.payload,
            )
            .await
            .as_ref()
            .map(provider_quota::quota_to_balance_status)
        };
        let breaker_open = provider_runtime::read_runtime_breaker_open(
            &state.redis_pool,
            &candidate.provider_account_id,
            candidate.provider_credential_id.as_deref(),
        )
        .await;
        health_snapshots.insert(
            candidate.runtime_subject_id().to_string(),
            HealthSnapshot {
                status: "active".to_string(),
                active_concurrency,
                failure_count: candidate.failure_count,
                breaker_open,
                balance_status,
            },
        );
    }
    let mut queue = build_candidate_queue(
        &all_candidates,
        &selection_strategy,
        None,
        &health_snapshots,
        ctx.route_policy_config
            .as_ref()
            .and_then(|config| config.provider_max_concurrent_requests)
            .map(|value| value.max(0) as usize),
    );

    for candidate in &mut queue {
        candidate.resolved_execution_mode = candidate
            .payload
            .resolve_execution_mode(ctx.canonical_req.endpoint_kind);
    }

    ctx.candidates = queue;
    ctx.route_selection_strategy = Some(selection_strategy);
    Ok(())
}

fn map_account_group_selection_error(
    error: crate::routing::config::RouteAccountGroupSelectionError,
) -> GatewayError {
    match error {
        crate::routing::config::RouteAccountGroupSelectionError::NotFound(group_id) => {
            GatewayError::bad_request(format!(
                "Requested account group '{}' was not found",
                group_id
            ))
            .with_code("account_group_not_found")
        }
        crate::routing::config::RouteAccountGroupSelectionError::Disabled(group_id) => {
            GatewayError::bad_request(format!(
                "Requested account group '{}' is disabled",
                group_id
            ))
            .with_code("account_group_disabled")
        }
    }
}

fn account_group_candidates_unavailable_error(
    constraint: &RouteAccountGroupConstraint,
    model: Option<&str>,
) -> Option<GatewayError> {
    let account_group_id = constraint.requested_group_id()?;
    let model_label = model.unwrap_or("<none>");
    Some(
        GatewayError::bad_request(format!(
            "No providers/accounts configured for requested account group '{}' and model '{}'",
            account_group_id, model_label
        ))
        .with_code("account_group_candidates_unavailable"),
    )
}

fn filter_candidate_pairs_by_account_group<T>(
    constraint: &RouteAccountGroupConstraint,
    candidates: Vec<RouteCandidate>,
    rows: Vec<T>,
) -> (Vec<RouteCandidate>, Vec<T>) {
    candidates
        .into_iter()
        .zip(rows)
        .filter(|(candidate, _)| constraint.allows_candidate(candidate))
        .unzip()
}

fn finalize_candidate_pairs_for_request<T: Clone>(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidates: Vec<RouteCandidate>,
    projected_rows: Option<Vec<T>>,
) -> (Vec<RouteCandidate>, Option<Vec<T>>) {
    let mut same_family = Vec::new();
    let mut others = Vec::new();
    match projected_rows {
        Some(rows) => {
            for (mut candidate, row) in candidates.into_iter().zip(rows.into_iter()) {
                let Some(is_same_family) = finalize_candidate_protocol_family(&mut candidate, req)
                else {
                    continue;
                };
                if is_same_family {
                    same_family.push((candidate, row));
                } else {
                    others.push((candidate, row));
                }
            }
            same_family.extend(others);
            let (candidates, rows): (Vec<_>, Vec<_>) = same_family.into_iter().unzip();
            (candidates, Some(rows))
        }
        None => {
            let mut same_only = Vec::new();
            let mut other_only = Vec::new();
            for mut candidate in candidates {
                let Some(is_same_family) = finalize_candidate_protocol_family(&mut candidate, req)
                else {
                    continue;
                };
                if is_same_family {
                    same_only.push(candidate);
                } else {
                    other_only.push(candidate);
                }
            }
            same_only.extend(other_only);
            (same_only, None)
        }
    }
}

fn filter_candidates_by_route_policy_family(
    candidates: Vec<RouteCandidate>,
    route_policy: Option<&crate::db::GatewayRoutePolicyConfig>,
) -> Vec<RouteCandidate> {
    let allowed_protocol_families = route_policy
        .and_then(|policy| policy.allowed_protocol_families.as_ref())
        .cloned();
    let Some(allowed_protocol_families) = allowed_protocol_families else {
        return candidates;
    };
    candidates
        .into_iter()
        .filter(|candidate| {
            allowed_protocol_families.iter().any(|allowed| {
                route_policy_family_matches_candidate(allowed, &candidate.protocol_family)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency::aimd::AimdConfig;
    use crate::concurrency::registry::ConcurrencyRegistry;
    use crate::config::Config;
    use crate::pipeline::PipelineContext;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use crate::redis::credential_cache::{CredentialEntry, CredentialKind};
    use crate::routing::config::RouteConfigStore;
    use crate::upstream::client::UpstreamClient;
    use std::collections::HashMap;

    fn make_config() -> Config {
        Config {
            console: Default::default(),
            runtime_role: crate::config::GatewayRuntimeRole::Standalone,
            port: 4200,
            redis_url: "redis://localhost".to_string(),
            database_url: None,
            upstream_timeout_secs: 30,
            max_request_body_bytes: 1024 * 1024,
            max_body_chat_completions_bytes: 1024 * 1024,
            max_body_completions_bytes: 1024 * 1024,
            max_body_messages_bytes: 1024 * 1024,
            max_body_responses_bytes: 1024 * 1024,
            max_body_embeddings_bytes: 1024 * 1024,
            max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
            max_body_audio_speech_bytes: 1024 * 1024,
            max_body_search_bytes: 512 * 1024,
            max_body_fetch_bytes: 512 * 1024,
            max_body_research_bytes: 512 * 1024,
            max_body_images_generations_bytes: 1024 * 1024,
            max_body_images_edits_bytes: 1024 * 1024,
            max_body_music_bytes: 1024 * 1024,
            max_body_videos_bytes: 1024 * 1024,
            response_cache_ttl_secs: 300,
            response_cache_max_size_bytes: 512 * 1024,
            quota_pre_deduct_estimate_ratio: 1.2,
            usage_report_batch_size: 100,
            provider_probe_interval_secs: 30,
            log_level: "info".to_string(),
            gateway_api_key: None,
            gateway_api_key_secret: None,
            gateway_management_token: None,
            gateway_keepalive_bearer_token: None,
            default_project_id: "platform-default-project".to_string(),
            gateway_inbound_api_key_header_aliases: Vec::new(),
            provider_credential_folder_sync_enabled: false,
            provider_credential_folder_sync_root_dir: None,
            provider_credential_folder_sync_interval_secs: 30,
            provider_credential_folder_sync_import_enabled: true,
            provider_credential_folder_sync_export_enabled: true,
            provider_credential_folder_sync_watch_enabled: true,
            provider_credential_folder_sync_watch_debounce_millis: 1500,
            provider_credential_folder_sync_delete_missing: false,
            provider_credential_refresh_enabled: true,
            provider_credential_refresh_interval_secs: 3600,
            provider_credential_refresh_before_secs: 86_400,
            provider_credential_refresh_batch_limit: 100,
            provider_credential_refresh_lock_ttl_secs: 300,
            credential_stock_monitor_enabled: true,
            credential_stock_monitor_interval_secs: 60,
            splitter_worker_executable_path: None,
            splitter_initial_worker_port: 4201,
            splitter_ready_timeout_secs: 120,
            splitter_ready_poll_interval_millis: 500,
            splitter_reload_shutdown_timeout_secs: 600,
        }
    }

    fn test_console_auth_runtime() -> Arc<crate::console::ConsoleAuthRuntime> {
        let temp = std::env::temp_dir().join(format!(
            "gateway-stage-route-console-{}",
            uuid::Uuid::new_v4()
        ));
        let console =
            crate::console::ConsoleConfig::from_values(crate::console::ConsoleConfigValues {
                state_dir: Some(temp.clone()),
                routes_file: Some(temp.join("routes.yaml")),
                ..Default::default()
            })
            .unwrap();
        Arc::new(crate::console::ConsoleAuthRuntime::new(&console, None).unwrap())
    }

    fn make_state() -> Arc<AppState> {
        make_state_with_route_store(RouteConfigStore::new())
    }

    fn make_state_with_route_store(route_config: RouteConfigStore) -> Arc<AppState> {
        Arc::new(AppState {
            config: make_config(),
            redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("pool"),
            pg_pool: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config: None,
            route_config: Arc::new(route_config),
            route_config_runtime: None,
            console_auth: test_console_auth_runtime(),
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
        })
    }

    fn make_cached_credential(id: &str) -> CredentialEntry {
        CredentialEntry {
            id: id.to_string(),
            kind: CredentialKind::PlatformUnlimited,
            project_id: "default".to_string(),
            user_id: "platform".to_string(),
            provider: "openai".to_string(),
            api_key: Some(format!("{id}-key")),
            api_base_url: Some("https://redis.example.com".to_string()),
            headers: None,
            account_payload: None,
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_request() -> CanonicalRelayRequest {
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

    #[tokio::test]
    async fn route_stage_errors_when_no_candidates() {
        let state = make_state();
        let mut ctx = PipelineContext::new(make_request(), None);
        // With an empty RouteConfigStore and no Redis credentials → should error.
        let result = run(&mut ctx, &state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn route_stage_succeeds_with_configured_routes() {
        // Write a temp YAML config file for the test.
        let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-test"
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default]
    priority: 10
"#;
        let tmp_dir = std::env::temp_dir();
        let tmp_path = tmp_dir.join("gw_test_routes_dual.yaml");
        std::fs::write(&tmp_path, yaml).unwrap();

        let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
        let state = Arc::new(AppState {
            config: make_config(),
            redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("pool"),
            pg_pool: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config: None,
            route_config: Arc::new(store),
            route_config_runtime: None,
            console_auth: test_console_auth_runtime(),
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
        });

        let mut ctx = PipelineContext::new(make_request(), None);
        let result = run(&mut ctx, &state).await;
        assert!(
            result.is_ok(),
            "stage_route should succeed: {:?}",
            result.err()
        );
        assert!(
            !ctx.candidates.is_empty(),
            "should have at least one candidate"
        );
        assert_eq!(ctx.candidates[0].provider_account_id, "openai-default");

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[tokio::test]
    async fn dual_resolution_falls_back_to_yaml_when_redis_empty() {
        // YAML provides routes; Redis has no credentials.
        // The stage should still resolve from YAML successfully.
        let yaml = r#"
providers:
  - id: anthropic-fallback
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "sk-ant-test"
model_routes:
  - pattern: "claude-*"
    provider_ids: [anthropic-fallback]
    priority: 10
"#;
        let tmp_dir = std::env::temp_dir();
        let tmp_path = tmp_dir.join("gw_test_dual_fallback.yaml");
        std::fs::write(&tmp_path, yaml).unwrap();

        let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
        let state = Arc::new(AppState {
            config: make_config(),
            redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("pool"),
            pg_pool: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config: None,
            route_config: Arc::new(store),
            route_config_runtime: None,
            console_auth: test_console_auth_runtime(),
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
        });

        // Request for claude model — YAML should serve it
        let mut req = make_request();
        req.requested_model = Some("claude-sonnet-4-6".to_string());
        let mut ctx = PipelineContext::new(req, None);

        let result = run(&mut ctx, &state).await;
        assert!(
            result.is_ok(),
            "should fall back to YAML: {:?}",
            result.err()
        );
        assert_eq!(ctx.candidates[0].provider_account_id, "anthropic-fallback");

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[tokio::test]
    async fn route_stage_filters_yaml_candidates_by_requested_account_group() {
        let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-openai"
    supported_models: [gpt-5.4]
  - id: codex-main
    preset: codex
    base_url: "https://muyuan.do/v1"
    api_key: ""
    supported_models: [gpt-5.4]
    credentials:
      - id: codex-live
        api_key: "sk-codex"
      - id: codex-spare
        api_key: "sk-codex-2"
account_groups:
  - id: premium
    name: "Premium"
    provider_credential_ids: [codex-live]
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default, codex-main]
    priority: 10
"#;
        let tmp_dir = std::env::temp_dir();
        let tmp_path = tmp_dir.join("gw_test_account_group_filter.yaml");
        std::fs::write(&tmp_path, yaml).unwrap();

        let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
        let state = Arc::new(AppState {
            config: make_config(),
            redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("pool"),
            pg_pool: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config: None,
            route_config: Arc::new(store),
            route_config_runtime: None,
            console_auth: test_console_auth_runtime(),
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
        });

        let mut req = make_request();
        req.requested_model = Some("gpt-5.4".to_string());
        let mut ctx = PipelineContext::new(req, None);
        ctx.account_group_id = Some("premium".to_string());

        let result = run(&mut ctx, &state).await;
        assert!(
            result.is_ok(),
            "grouped route should succeed: {:?}",
            result.err()
        );
        assert_eq!(ctx.candidates.len(), 1);
        assert_eq!(ctx.candidates[0].provider_account_id, "codex-main");
        assert_eq!(
            ctx.candidates[0].payload.credential_id.as_deref(),
            Some("codex-live")
        );

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[tokio::test]
    async fn route_stage_errors_for_unknown_requested_account_group() {
        let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-openai"
    supported_models: [gpt-5.4]
model_routes:
  - pattern: "gpt-*"
    provider_ids: [openai-default]
    priority: 10
"#;
        let tmp_dir = std::env::temp_dir();
        let tmp_path = tmp_dir.join("gw_test_account_group_missing.yaml");
        std::fs::write(&tmp_path, yaml).unwrap();

        let store = RouteConfigStore::load_from_yaml(&tmp_path).unwrap();
        let state = Arc::new(AppState {
            config: make_config(),
            redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("pool"),
            pg_pool: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config: None,
            route_config: Arc::new(store),
            route_config_runtime: None,
            console_auth: test_console_auth_runtime(),
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
        });

        let mut req = make_request();
        req.requested_model = Some("gpt-5.4".to_string());
        let mut ctx = PipelineContext::new(req, None);
        ctx.account_group_id = Some("missing".to_string());

        let error = run(&mut ctx, &state)
            .await
            .expect_err("missing group should fail");
        assert_eq!(error.code.as_deref(), Some("account_group_not_found"));

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn access_catalog_candidate_pairs_are_group_filtered_without_losing_row_alignment() {
        let store = RouteConfigStore::from_document(
            serde_yaml::from_str(
                r#"
providers:
  - id: access-provider
    base_url: "https://example.com"
    credentials:
      - { id: access-live, api_key: live-key }
      - { id: access-outside, api_key: outside-key }
account_groups:
  - id: isolated
    name: Isolated
    provider_credential_ids: [access-live]
model_routes: []
"#,
            )
            .expect("route document"),
        )
        .expect("route store");
        let template = store
            .resolve_candidates(None)
            .into_iter()
            .next()
            .expect("candidate template");
        let mut outside = template.clone();
        outside.provider_credential_id = Some("access-outside".to_string());
        outside.payload.credential_id = Some("access-outside".to_string());
        let mut live = template;
        live.provider_credential_id = Some("access-live".to_string());
        live.payload.credential_id = Some("access-live".to_string());
        let constraint = store
            .account_group_constraint(Some("isolated"))
            .expect("account group");

        let (candidates, rows) = filter_candidate_pairs_by_account_group(
            &constraint,
            vec![outside, live],
            vec!["outside-row", "live-row"],
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].payload.credential_id.as_deref(),
            Some("access-live")
        );
        assert_eq!(rows, vec!["live-row"]);
    }

    #[tokio::test]
    async fn route_stage_filters_cached_redis_candidates_before_queue_selection() {
        let store = RouteConfigStore::from_document(
            serde_yaml::from_str(
                r#"
providers:
  - id: yaml-provider
    base_url: "https://yaml.example.com"
    supported_models: [gpt-5.4]
    credentials:
      - { id: yaml-live, api_key: yaml-key }
account_groups:
  - id: isolated
    name: Isolated
    provider_credential_ids: [yaml-live]
model_routes:
  - pattern: gpt-5.4
    provider_ids: [yaml-provider]
    priority: 10
"#,
            )
            .expect("route document"),
        )
        .expect("route store");
        let state = make_state_with_route_store(store);
        state.credential_cache.put(
            "default",
            "gpt-5.4",
            vec![make_cached_credential("redis-outside")],
        );
        let mut request = make_request();
        request.requested_model = Some("gpt-5.4".to_string());
        let mut ctx = PipelineContext::new(request, None);
        ctx.account_group_id = Some("isolated".to_string());

        run(&mut ctx, &state).await.expect("grouped route");

        assert_eq!(ctx.candidates.len(), 1);
        assert_eq!(
            ctx.candidates[0].payload.credential_id.as_deref(),
            Some("yaml-live")
        );
    }

    #[tokio::test]
    async fn route_stage_never_falls_back_to_an_out_of_group_cached_credential() {
        let store = RouteConfigStore::from_document(
            serde_yaml::from_str(
                r#"
providers:
  - id: yaml-provider
    base_url: "https://yaml.example.com"
    supported_models: [gpt-5.4]
    credentials:
      - id: yaml-disabled
        api_key: disabled-key
        enabled: false
account_groups:
  - id: isolated
    name: Isolated
    provider_credential_ids: [yaml-disabled]
model_routes:
  - pattern: gpt-5.4
    provider_ids: [yaml-provider]
    priority: 10
"#,
            )
            .expect("route document"),
        )
        .expect("route store");
        let state = make_state_with_route_store(store);
        state.credential_cache.put(
            "default",
            "gpt-5.4",
            vec![make_cached_credential("redis-outside")],
        );
        let mut request = make_request();
        request.requested_model = Some("gpt-5.4".to_string());
        let mut ctx = PipelineContext::new(request, None);
        ctx.account_group_id = Some("isolated".to_string());

        let error = run(&mut ctx, &state)
            .await
            .expect_err("out-of-group Redis credential must not be used");

        assert_eq!(
            error.code.as_deref(),
            Some("account_group_candidates_unavailable")
        );
    }
}
