//! One lock owns each generation of OAuth material; policy wrappers never reset its lifecycle.
use super::*;
use crate::protocol::grok_oauth::AuthorizationContext;

pub(super) struct RefreshAuthorization {
    pub endpoint: String,
    pub client_id: String,
    pub attempt_lock: Arc<tokio::sync::Mutex<()>>,
    pub material: parking_lot::Mutex<RefreshMaterial>,
}

pub(super) struct RefreshMaterial {
    pub active: bool,
    pub generation: u64,
    pub failures: u32,
    pub retry_at: Option<std::time::Instant>,
    pub reauthorization_required: bool,
    pub refresh_token: String,
    pub api_key_override: Option<String>,
    pub grok_oauth: Option<AuthorizationContext>,
    pub kiro_oauth: Option<crate::protocol::kiro_oauth::AuthorizationContext>,
    pub vertex_oauth: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
    pub dim_oauth: Option<crate::protocol::dim_oauth::AuthorizationContext>,
    pub nous_portal: Option<crate::protocol::nous_portal::AuthorizationContext>,
    pub lobsterai: Option<crate::protocol::lobsterai::AuthorizationContext>,
    pub accio_sgcookie: Option<String>,
    pub expires_at: Option<std::time::Instant>,
    pub rejected: bool,
    pub recovery_checked: bool,
    pub expires_at_iso: Option<String>,
    pub expires_in_secs: Option<u64>,
}

/// Refresh expiry never inherits the provider payload's session/expiry defaults.
pub(crate) fn refresh_absolute_expiry(cred: &ProviderCredentialYaml) -> Option<String> {
    let value = cred.expires_at.as_deref().or_else(|| {
        cred.session_auth
            .as_ref()
            .and_then(|s| s.expires_at.as_deref())
    })?;
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .ok()?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

fn remaining_seconds(expiry: Option<&str>) -> u64 {
    expiry
        .and_then(|value| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
        })
        .map(|at| {
            (at - time::OffsetDateTime::now_utc())
                .whole_seconds()
                .max(0) as u64
        })
        .unwrap_or(0)
}

impl RefreshAuthorization {
    pub fn from_credential(cred: &ProviderCredentialYaml) -> Option<Self> {
        let refresh_token = cred
            .refresh_token
            .as_ref()
            .filter(|s| !s.trim().is_empty())?;
        let (endpoint, client_id) = if let Some(context) = &cred.lobsterai {
            context
                .validate_binding(cred.api_key.as_deref()?, refresh_token)
                .ok()?;
            (
                crate::protocol::lobsterai::TOKEN_ENDPOINT.into(),
                crate::protocol::lobsterai::CLIENT_ATTRIBUTION.into(),
            )
        } else if let Some(context) = &cred.nous_portal {
            context.validate().ok()?;
            (
                crate::protocol::nous_portal::TOKEN_ENDPOINT.into(),
                context.client_id().into(),
            )
        } else if let Some(context) = &cred.dim_oauth {
            context.validate().ok()?;
            (context.token_endpoint().into(), context.client_id().into())
        } else if let Some(context) = &cred.vertex_oauth {
            context.validate().ok()?;
            (
                crate::protocol::vertex_oauth::TOKEN_ENDPOINT.to_owned(),
                context.client_id().to_owned(),
            )
        } else if let Some(context) = &cred.grok_oauth {
            context.validate().ok()?;
            (
                crate::protocol::grok_oauth::TOKEN_ENDPOINT.to_owned(),
                context.client_id().to_owned(),
            )
        } else if let Some(context) = &cred.kiro_oauth {
            context.validate().ok()?;
            (context.token_endpoint(), context.client_id().to_owned())
        } else {
            (
                cred.refresh_endpoint
                    .as_ref()
                    .filter(|s| !s.trim().is_empty())?
                    .clone(),
                cred.refresh_client_id.clone().unwrap_or_default(),
            )
        };
        let expiry = refresh_absolute_expiry(cred);
        // An unknown absolute expiry is due; rebuilding a policy wrapper cannot grant a fresh lifetime.
        let remaining = remaining_seconds(expiry.as_deref());
        Some(Self {
            endpoint,
            client_id,
            attempt_lock: Arc::new(tokio::sync::Mutex::new(())),
            material: parking_lot::Mutex::new(RefreshMaterial {
                active: true,
                generation: 0,
                failures: 0,
                retry_at: None,
                reauthorization_required: false,
                refresh_token: refresh_token.clone(),
                api_key_override: cred.lobsterai.as_ref().and(cred.api_key.clone()),
                grok_oauth: cred.grok_oauth.clone(),
                kiro_oauth: cred.kiro_oauth.clone(),
                vertex_oauth: cred.vertex_oauth.clone(),
                dim_oauth: cred.dim_oauth.clone(),
                nous_portal: cred.nous_portal.clone(),
                lobsterai: cred.lobsterai.clone(),
                accio_sgcookie: cred.accio_sgcookie.clone(),
                expires_at: cred
                    .lobsterai
                    .is_none()
                    .then(|| safe_refresh_deadline(remaining)),
                rejected: false,
                recovery_checked: cred.lobsterai.is_none(),
                expires_at_iso: expiry,
                expires_in_secs: cred.lobsterai.is_none().then(|| {
                    effective_refresh_lifetime_secs(cred.token_expires_in_secs.unwrap_or(21600))
                }),
            }),
        })
    }

    pub fn apply(
        &self,
        generation: u64,
        access: String,
        refresh: Option<String>,
        lifetime: Option<u64>,
        expiry: Option<String>,
        context: Option<AuthorizationContext>,
        sgcookie: Option<String>,
        vertex: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
        dim: Option<crate::protocol::dim_oauth::AuthorizationContext>,
        nous: Option<crate::protocol::nous_portal::AuthorizationContext>,
        lobster: Option<crate::protocol::lobsterai::AuthorizationContext>,
    ) -> Option<u64> {
        let mut state = self.material.lock();
        if !state.active || state.generation != generation {
            return None;
        }
        state.api_key_override = Some(access);
        if let Some(refresh) = refresh {
            state.refresh_token = refresh;
        }
        if let Some(context) = context {
            state.grok_oauth = Some(context);
        }
        if let Some(cookie) = sgcookie {
            state.accio_sgcookie = Some(cookie);
        }
        if let Some(context) = vertex {
            state.vertex_oauth = Some(context);
        }
        if let Some(context) = dim {
            state.dim_oauth = Some(context);
        }
        if let Some(context) = nous {
            state.nous_portal = Some(context);
        }
        // Delayed durable replay must not grant the token another full lifetime.
        if let Some(context) = lobster {
            state.lobsterai = Some(context);
        }
        state.expires_at = expiry
            .as_deref()
            .map(|expiry| safe_refresh_deadline(remaining_seconds(Some(expiry))));
        state.expires_at_iso = expiry;
        state.rejected = false;
        state.recovery_checked = true;
        state.expires_in_secs = lifetime;
        state.generation = state.generation.wrapping_add(1);
        state.failures = 0;
        state.retry_at = None;
        state.reauthorization_required = false;
        Some(state.generation)
    }

    pub fn record_failure(&self, generation: u64, terminal: bool) {
        let mut state = self.material.lock();
        if !state.active || state.generation != generation {
            return;
        }
        state.failures = state.failures.saturating_add(1);
        state.reauthorization_required = terminal;
        let delay = 60u64.saturating_mul(1 << state.failures.saturating_sub(1).min(6));
        state.retry_at = Some(safe_refresh_deadline(delay.min(3600)));
    }
}
