//! A started exchange pins one authorization generation, independent of later routing-policy retirement.
use super::*;
use crate::protocol::grok_oauth::AuthorizationContext;

pub(crate) struct RefreshAttempt {
    authorization: Arc<refresh_material::RefreshAuthorization>,
    pub generation: u64,
    pub endpoint: String,
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub context: Option<AuthorizationContext>,
    pub kiro_oauth: Option<crate::protocol::kiro_oauth::AuthorizationContext>,
    pub vertex_oauth: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
    pub dim_oauth: Option<crate::protocol::dim_oauth::AuthorizationContext>,
    pub nous_portal: Option<crate::protocol::nous_portal::AuthorizationContext>,
    pub lobsterai: Option<crate::protocol::lobsterai::AuthorizationContext>,
    pub accio_sgcookie: Option<String>,
    pub expires_at: Option<String>,
    pub guard: tokio::sync::OwnedMutexGuard<()>,
}

impl TokenRefreshState {
    pub(crate) fn begin_refresh(
        &self,
        margin: std::time::Duration,
        fallback_access: &str,
    ) -> Option<RefreshAttempt> {
        let guard = self.try_refresh_lock()?;
        let state = self.authorization.material.lock();
        if !self.routing_active.load(Ordering::Acquire)
            || !state.active
            || state.reauthorization_required
            || state
                .retry_at
                .is_some_and(|at| at > std::time::Instant::now())
            || !state.recovery_checked
            || match state.expires_at {
                Some(at) => at.saturating_duration_since(std::time::Instant::now()) > margin,
                None => !state.rejected,
            }
        {
            return None;
        }
        Some(RefreshAttempt {
            authorization: self.authorization.clone(),
            generation: state.generation,
            endpoint: self.authorization.endpoint.clone(),
            client_id: self.authorization.client_id.clone(),
            access_token: state
                .api_key_override
                .as_deref()
                .unwrap_or(fallback_access)
                .to_owned(),
            refresh_token: state.refresh_token.clone(),
            context: state.grok_oauth.clone(),
            kiro_oauth: state.kiro_oauth.clone(),
            vertex_oauth: state.vertex_oauth.clone(),
            dim_oauth: state.dim_oauth.clone(),
            nous_portal: state.nous_portal.clone(),
            lobsterai: state.lobsterai.clone(),
            accio_sgcookie: state.accio_sgcookie.clone(),
            expires_at: state.expires_at_iso.clone(),
            guard,
        })
    }

    pub(crate) fn authorization_is_current(&self, other: &Self, generation: u64) -> bool {
        if !Arc::ptr_eq(&self.authorization, &other.authorization) {
            return false;
        }
        let state = self.authorization.material.lock();
        state.active && state.generation == generation
    }

    pub(super) fn authorization_id(&self) -> usize {
        Arc::as_ptr(&self.authorization) as usize
    }

    /// Only the exact currently published rotation may advance the document anchor.
    pub(super) fn persisted_rotation_matches(
        &self,
        previous: &ProviderCredentialYaml,
        next: &ProviderCredentialYaml,
        effective_access: &str,
        effective_base: &str,
        previous_base: &str,
    ) -> bool {
        let state = self.authorization.material.lock();
        if !state.active || state.api_key_override.as_deref() != Some(effective_access) {
            return false;
        }
        let mut expected = previous.clone();
        expected.api_key = state.api_key_override.clone();
        expected.refresh_token = Some(state.refresh_token.clone());
        expected.grok_oauth = state.grok_oauth.clone();
        expected.vertex_oauth = state.vertex_oauth.clone();
        expected.dim_oauth = state.dim_oauth.clone();
        expected.nous_portal = state.nous_portal.clone();
        expected.lobsterai = state.lobsterai.clone();
        if let Some(context) = &state.nous_portal {
            if effective_base.trim_end_matches('/') != context.inference_target().base_url() {
                return false;
            }
            if previous
                .nous_portal
                .as_ref()
                .is_some_and(|old| old.inference_target() != context.inference_target())
            {
                expected.base_url = Some(context.inference_target().base_url().into());
            }
            if expected.base_url != next.base_url {
                return false;
            }
        } else if effective_base != previous_base {
            return false;
        }
        expected.accio_sgcookie = state.accio_sgcookie.clone();
        expected.expires_at = state.expires_at_iso.clone();
        expected.token_expires_in_secs = state.expires_in_secs;
        if let Some(session) = &mut expected.session_auth {
            session.expires_at = state.expires_at_iso.clone();
        }
        same_authorization(&expected, next)
    }
}

impl RefreshAttempt {
    pub fn apply_lobster(
        &self,
        tokens: &crate::protocol::lobsterai::SessionMaterial,
    ) -> Option<u64> {
        let old = self.lobsterai.as_ref()?;
        if !old.permits_rotation(&tokens.context)
            || tokens
                .context
                .validate_binding(&tokens.access_token, &tokens.refresh_token)
                .is_err()
        {
            return None;
        }
        self.authorization.apply(
            self.generation,
            tokens.access_token.clone(),
            Some(tokens.refresh_token.clone()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(tokens.context.clone()),
        )
    }

    pub fn apply(
        &self,
        access: String,
        refresh: Option<String>,
        lifetime: u64,
        expiry: String,
        context: Option<AuthorizationContext>,
        sgcookie: Option<String>,
        vertex: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
    ) -> Option<u64> {
        self.apply_with_dim(
            access, refresh, lifetime, expiry, context, sgcookie, vertex, None,
        )
    }
    pub fn apply_with_dim(
        &self,
        access: String,
        refresh: Option<String>,
        lifetime: u64,
        expiry: String,
        context: Option<AuthorizationContext>,
        sgcookie: Option<String>,
        vertex: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
        dim: Option<crate::protocol::dim_oauth::AuthorizationContext>,
    ) -> Option<u64> {
        self.apply_with_nous(
            access, refresh, lifetime, expiry, context, sgcookie, vertex, dim, None,
        )
    }
    pub fn apply_with_nous(
        &self,
        access: String,
        refresh: Option<String>,
        lifetime: u64,
        expiry: String,
        context: Option<AuthorizationContext>,
        sgcookie: Option<String>,
        vertex: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
        dim: Option<crate::protocol::dim_oauth::AuthorizationContext>,
        nous: Option<crate::protocol::nous_portal::AuthorizationContext>,
    ) -> Option<u64> {
        self.authorization.apply(
            self.generation,
            access,
            refresh,
            Some(lifetime),
            Some(expiry),
            context,
            sgcookie,
            vertex,
            dim,
            nous,
            None,
        )
    }

    pub fn record_failure(&self, terminal: bool) {
        self.authorization.record_failure(self.generation, terminal);
    }
}
