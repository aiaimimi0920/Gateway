//! Recheck mutable upstream dispatch state; never change caller authorization or consume attempts.
use crate::{error::GatewayError, routing::candidate::RouteCandidate, state::AppState};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct EligibilityCheck {
    state: Arc<AppState>,
    candidate: RouteCandidate,
    config_owned: bool,
}

impl EligibilityCheck {
    pub(crate) fn new(state: &Arc<AppState>, candidate: &RouteCandidate, model: &str) -> Self {
        let config_owned = candidate.provider_credential_id.is_none()
            && state
                .route_config
                .snapshot()
                .credential_dispatch_allowed(
                    &candidate.provider_account_id,
                    candidate.payload.credential_id.as_deref(),
                )
                .is_some();
        let mut candidate = candidate.clone();
        candidate.upstream_model = Some(model.to_owned());
        Self {
            state: Arc::clone(state),
            candidate,
            config_owned,
        }
    }

    pub(crate) async fn check(&self) -> Result<(), GatewayError> {
        let candidate = &self.candidate;
        let config = self.state.route_config.snapshot();
        crate::routing::admission::check_candidate(
            &self.state,
            candidate,
            candidate.upstream_model.as_deref().unwrap_or_default(),
        )?;
        if self.config_owned {
            if config.credential_dispatch_allowed(
                &candidate.provider_account_id,
                candidate.payload.credential_id.as_deref(),
            ) != Some(true)
            {
                return Err(unavailable());
            }
        } else if let Some(pool) = &self.state.pg_pool {
            // Status-only reads avoid loading credential secrets or running recovery writers.
            let account: Option<(bool, bool)> = sqlx::query_as(
                "SELECT status = 'active' AND (cooldown_until IS NULL OR cooldown_until <= now()),
                 EXISTS(SELECT 1 FROM gateway_provider_credentials c WHERE c.provider_account_id = a.id)
                 FROM gateway_provider_accounts a WHERE a.id = $1")
                .bind(&candidate.provider_account_id).fetch_optional(pool).await
                .map_err(|_| GatewayError::server_error("Cannot verify upstream dispatch state"))?;
            if account.is_some_and(|(active, has_pool)| {
                !active || (has_pool && candidate.provider_credential_id.is_none())
            }) {
                return Err(unavailable());
            }
            if let Some(id) = candidate.provider_credential_id.as_deref() {
                let active: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM gateway_provider_credentials WHERE id = $1
                     AND provider_account_id = $2 AND status = 'active' AND archived_at IS NULL
                     AND (cooldown_until IS NULL OR cooldown_until <= now()))",
                )
                .bind(id)
                .bind(&candidate.provider_account_id)
                .fetch_one(pool)
                .await
                .map_err(|_| {
                    GatewayError::server_error("Cannot verify upstream credential state")
                })?;
                if account.is_none() || !active {
                    return Err(unavailable());
                }
            }
        }
        // Persisted authentication failures must not be resurrected by session stickiness.
        let mut current = vec![candidate.clone()];
        if let Some(local) = &self.state.local_runtime {
            local.apply_model_states(&mut current).await?;
        } else if let Some(pool) = &self.state.pg_pool {
            crate::db::apply_provider_credential_model_states_to_candidates(pool, &mut current)
                .await?;
        }
        if current[0]
            .routing_degradation_reasons
            .iter()
            .any(|reason| reason == "credential_model_blocked")
        {
            return Err(unavailable());
        }
        Ok(())
    }
}

fn unavailable() -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "Upstream credential is disabled or unavailable; select another eligible candidate",
    )
    .with_code("upstream_credential_unavailable");
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::FallbackProvider {
        reason: error.message.clone(),
    };
    error
}
