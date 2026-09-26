// ---------------------------------------------------------------------------
// Pipeline stage 3 — route candidate resolution (dual resolution)
//
// Resolves the ordered list of upstream provider candidates for this request.
// Resolution order:
//   1. Redis credentials (user-owned via X-Credential-Ref, then project pool)
//   2. YAML static config (fallback)
//   3. Error if both sources yield zero candidates
// ---------------------------------------------------------------------------

use std::sync::Arc;

use tracing::{debug, warn};

use crate::db;
use crate::error::GatewayError;
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
use crate::state::AppState;

use super::stage_route_health::collect_candidate_health_snapshots;
use super::{CredentialSource, PipelineContext};

mod account_group;
mod candidate_policy;

use account_group::{
    account_group_candidates_unavailable_error, filter_candidate_pairs_by_account_group,
    map_account_group_selection_error,
};
use candidate_policy::{
    filter_candidates_by_route_policy_family, finalize_candidate_pairs_for_request,
};

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
        let database_resolved_candidates = if let Some(pg_pool) = &state.pg_pool {
            provider_runtime::sweep_cooling_provider_accounts_best_effort(state, 10).await;
            let route_context = crate::db::resolve_route_candidates_allowing_empty(
                pg_pool,
                project_id,
                model,
                ctx.canonical_req.endpoint_kind,
            )
            .await?;
            ctx.route_policy_id = Some(route_context.route_policy_id);
            ctx.route_policy_config = Some(route_context.route_policy_config.clone());
            selection_strategy = route_context.selection_strategy;
            let resolved_count = route_context.candidates.len();
            all_candidates.extend(route_context.candidates);
            resolved_count > 0
        } else {
            false
        };

        // A standalone deployment keeps its providers in the route document,
        // while a Platform-managed deployment keeps them in
        // `gateway_provider_accounts`. PostgreSQL is also what backs the
        // console's audit, cost and concurrency panels, so configuring a
        // database must not make the route document unreachable. The route
        // document therefore stays the fallback whenever the database resolved
        // no candidate of its own.
        if !database_resolved_candidates {
            match state
                .route_config
                .resolve_candidates_for_account_group(model, requested_account_group_id.as_deref())
            {
                Ok(yaml_candidates) => all_candidates.extend(yaml_candidates),
                // With a database present the requested account group can be a
                // database-owned group the route document cannot know about, so
                // the database result stands and the usual "no provider
                // account" error is reported downstream.
                Err(error) if state.pg_pool.is_some() => {
                    debug!(
                        req_id = %ctx.req_id,
                        error = %error,
                        "route document account group unavailable; keeping database routing result"
                    );
                }
                Err(error) => return Err(map_account_group_selection_error(error)),
            }
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
    let (all_candidates, health_snapshots) = collect_candidate_health_snapshots(
        all_candidates,
        Arc::clone(state),
        concurrency_snapshots,
    )
    .await;
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

#[cfg(test)]
mod tests;
