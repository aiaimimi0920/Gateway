use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use tracing::warn;
use uuid::Uuid;

use crate::db;
use crate::error::GatewayError;
use crate::metrics::request::global_gateway_metrics;
use crate::rate_limit::{
    build_provider_attempt_rate_limit_rule, build_request_rate_limit_rules,
    endpoint_rate_limit_identity, enforce_rate_limit_rules, RateLimitAdmission,
    RateLimitDimensions, RedisRateLimitStore,
};
use crate::state::AppState;

use super::PipelineContext;
mod attempt_budget;

#[derive(Clone)]
pub struct ProviderAttemptGate {
    req_id: Uuid,
    redis_pool: crate::redis::pool::RedisPool,
    local_limits: Option<Arc<crate::rate_limit::MemoryRateLimitStore>>,
    rules: Vec<crate::rate_limit::RateLimitRule>,
    outbound_attempt_count: Arc<AtomicU32>,
    attempted_provider_ids: Arc<parking_lot::Mutex<Vec<String>>>,
    provider_account_id: String,
    request_budget: super::request_budget::RequestBudget,
}

impl ProviderAttemptGate {
    pub async fn admit(&self) -> Result<(), GatewayError> {
        self.request_budget.check()?;
        enforce(
            self.req_id,
            &self.redis_pool,
            self.local_limits.as_deref(),
            &self.rules,
        )
        .await
    }
}

pub async fn run(ctx: &mut PipelineContext, state: &Arc<AppState>) -> Result<(), GatewayError> {
    ensure_route_policy_loaded(ctx, state).await?;
    let Some(config) = ctx.route_policy_config.as_ref() else {
        return Ok(());
    };
    let dimensions = request_dimensions(ctx, None)?;
    let rules = build_request_rate_limit_rules(config, &dimensions)?;
    enforce(
        ctx.req_id,
        &state.redis_pool,
        state
            .local_runtime
            .as_ref()
            .map(|local| local.rate_limits.as_ref()),
        &rules,
    )
    .await
}

pub fn provider_attempt_gate(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    provider_account_id: &str,
) -> Result<ProviderAttemptGate, GatewayError> {
    let rules = if let Some(config) = ctx.route_policy_config.as_ref().filter(|config| {
        config.rate_limit_enforcement_version.as_deref() == Some("v1")
            && config.provider_attempt_rate_limit.is_some()
    }) {
        let dimensions = request_dimensions(ctx, Some(provider_account_id))?;
        build_provider_attempt_rate_limit_rule(config, &dimensions)?
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };
    Ok(ProviderAttemptGate {
        req_id: ctx.req_id,
        redis_pool: state.redis_pool.clone(),
        local_limits: state
            .local_runtime
            .as_ref()
            .map(|local| Arc::clone(&local.rate_limits)),
        rules,
        outbound_attempt_count: Arc::clone(&ctx.route_attempt_count),
        attempted_provider_ids: Arc::clone(&ctx.attempted_provider_ids),
        provider_account_id: provider_account_id.to_string(),
        request_budget: ctx.request_budget.clone(),
    })
}

async fn enforce(
    req_id: Uuid,
    redis_pool: &crate::redis::pool::RedisPool,
    local_limits: Option<&crate::rate_limit::MemoryRateLimitStore>,
    rules: &[crate::rate_limit::RateLimitRule],
) -> Result<(), GatewayError> {
    if rules.is_empty() {
        return Ok(());
    }
    let store = RedisRateLimitStore::new(redis_pool);
    let store: &dyn crate::rate_limit::RateLimitStore = local_limits
        .map_or(&store as &dyn crate::rate_limit::RateLimitStore, |local| {
            local
        });
    let admission = enforce_rate_limit_rules(store, rules).await;
    let rejected = matches!(admission, RateLimitAdmission::Rejected { .. });
    let store_failure = matches!(
        admission,
        RateLimitAdmission::BypassedStoreUnavailable { .. }
            | RateLimitAdmission::IndeterminateStoreFailure { .. }
    );
    global_gateway_metrics().observe_rate_limit_admission(rejected, store_failure);
    if let RateLimitAdmission::BypassedStoreUnavailable { reason } = &admission {
        warn!(
            req_id = %req_id,
            reason = %reason,
            "rate-limit store unavailable; allowing request under fail-open policy"
        );
    }
    if let RateLimitAdmission::IndeterminateStoreFailure { reason } = &admission {
        warn!(
            req_id = %req_id,
            reason = %reason,
            "rate-limit admission result indeterminate; rejecting request"
        );
    }
    admission.into_gateway_result()
}

async fn ensure_route_policy_loaded(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<(), GatewayError> {
    if ctx.route_policy_config.is_some() {
        return Ok(());
    }
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    let session = ctx.session.as_ref().ok_or_else(|| {
        GatewayError::server_error("rate-limit policy loading requires an authenticated session")
            .with_code("rate_limit_session_missing")
    })?;
    if let Some(route_policy) =
        db::find_active_route_policy_for_rate_limits(pg_pool, &session.project_id).await?
    {
        ctx.route_policy_id = Some(route_policy.id);
        ctx.route_policy_config = Some(route_policy.config);
    }
    Ok(())
}

fn request_dimensions<'a>(
    ctx: &'a PipelineContext,
    provider_account_id: Option<&'a str>,
) -> Result<RateLimitDimensions<'a>, GatewayError> {
    let session = ctx.session.as_ref().ok_or_else(|| {
        GatewayError::server_error("rate-limit admission requires an authenticated session")
            .with_code("rate_limit_session_missing")
    })?;
    let access_key_id = session
        .access_key_id
        .as_deref()
        .or(session.api_key_id.as_deref())
        .or(session.user_credential_id.as_deref());
    let (endpoint, endpoint_policy_alias) =
        endpoint_rate_limit_identity(ctx.canonical_req.endpoint_kind);
    Ok(RateLimitDimensions {
        tenant_id: &session.tenant_id,
        project_id: &session.project_id,
        access_key_id,
        model: ctx.canonical_req.requested_model.as_deref(),
        endpoint,
        endpoint_policy_alias: Some(endpoint_policy_alias),
        provider_account_id,
    })
}
