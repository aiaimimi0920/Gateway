//! A send ticket binds exactly the dispatched bearer and generation to the durable refresh owner.
use super::{metadata, owns_payload};
use crate::{
    error::GatewayError,
    routing::{
        candidate::{ProviderAccountPayload, RouteCandidate},
        config::TokenRefreshState,
    },
    state::AppState,
};
use std::sync::Arc;
pub(crate) struct Ticket {
    owner: Arc<TokenRefreshState>,
    generation: u64,
    access: String,
}
impl Ticket {
    pub(crate) fn capture(
        state: &AppState,
        candidate: &RouteCandidate,
        payload: &ProviderAccountPayload,
    ) -> Result<Option<Self>, GatewayError> {
        Self::capture_for_provider(state, &candidate.provider_account_id, payload)
    }
    pub(crate) fn capture_for_provider(
        state: &AppState,
        provider_id: &str,
        payload: &ProviderAccountPayload,
    ) -> Result<Option<Self>, GatewayError> {
        if !owns_payload(payload) {
            return Ok(None);
        }
        // Server's current refresh cache requires expiry; do not consume an expiry-less grant there.
        if state.local_runtime.is_none() {
            return Err(super::invalid());
        }
        let projected = metadata::read(payload)?;
        let snapshot = state.route_config.snapshot();
        let owner = payload
            .credential_id
            .as_deref()
            .and_then(|id| snapshot.oauth_refresh_target(provider_id, id))
            .and_then(|(_, cred)| cred.refresh_config.as_ref())
            .ok_or_else(super::invalid)?;
        owner.dispatch_readiness()?;
        let (access, context, generation) = owner
            .lobster_dispatch_snapshot()
            .ok_or_else(super::invalid)?;
        if projected.generation != generation
            || projected.context != context
            || payload.api_key != access
        {
            return Err(GatewayError::service_unavailable(
                "LobsterAI credential changed before dispatch",
            )
            .with_code("upstream_credential_cooling"));
        }
        Ok(Some(Self {
            owner: owner.clone(),
            generation,
            access,
        }))
    }
    pub(crate) fn observe(&self, state: &AppState, error: &GatewayError) {
        // Only our HTTP sender's 401 is evidence; synthesized cancellation/budget errors never enter here.
        if error.code.as_deref() == Some("lobsterai_upstream_rejected")
            && error.http_status == Some(401)
            && self
                .owner
                .reject_lobster_access(self.generation, &self.access, 401)
        {
            state.lifecycle.oauth_refresh_wake.notify_one();
        }
    }
}
