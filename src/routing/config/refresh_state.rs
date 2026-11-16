//! OAuth refresh deadlines, generation checks and retirement.

use super::*;

/// Validate declared lifetimes before legacy compilation can cap an invalid input.
pub(crate) fn token_expiry_supported(seconds: u64) -> bool {
    let duration = std::time::Duration::from_secs(seconds);
    std::time::Instant::now().checked_add(duration).is_some()
        && std::time::SystemTime::now().checked_add(duration).is_some()
}

pub(crate) fn effective_refresh_lifetime_secs(seconds: u64) -> u64 {
    seconds.min(MAX_REFRESH_LIFETIME_SECS)
}

pub fn system_time_to_rfc3339_millis(time: std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
    let millis = duration.subsec_millis();

    let days = total_secs / 86_400;
    let secs_of_day = total_secs % 86_400;
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hour, minute, second, millis
    )
}

pub fn future_rfc3339_after_secs(offset_secs: u64) -> String {
    let bounded = offset_secs.min(i64::MAX as u64);
    let target = std::time::SystemTime::now()
        .checked_add(std::time::Duration::from_secs(bounded))
        .unwrap_or_else(std::time::SystemTime::now);
    system_time_to_rfc3339_millis(target)
}

/// The maximum lifetime we use for an in-process `Instant` deadline.  Remote
/// OAuth servers occasionally return an unbounded or nonsensical `expires_in`;
/// capping the local deadline keeps refresh bookkeeping panic-free while still
/// treating the token as long-lived.
const MAX_REFRESH_LIFETIME_SECS: u64 = 100 * 365 * 24 * 60 * 60;

impl TokenRefreshState {
    pub(crate) fn lobster_dispatch_snapshot(
        &self,
    ) -> Option<(
        String,
        crate::protocol::lobsterai::AuthorizationContext,
        u64,
    )> {
        let state = self.authorization.material.lock();
        if !self.is_routing_active() || !state.active {
            return None;
        }
        Some((
            state.api_key_override.clone()?,
            state.lobsterai.clone()?,
            state.generation,
        ))
    }

    /// Capture alongside the bearer before dispatch; late 401s cannot invalidate a newer generation.
    pub(crate) fn lobster_dispatch_generation(&self) -> Option<u64> {
        let state = self.authorization.material.lock();
        state.lobsterai.as_ref().map(|_| state.generation)
    }
    pub(crate) fn reject_lobster_access(&self, generation: u64, access: &str, status: u16) -> bool {
        let mut state = self.authorization.material.lock();
        if status != 401
            || !self.is_routing_active()
            || !state.active
            || state.lobsterai.is_none()
            || state.generation != generation
            || state.api_key_override.as_deref() != Some(access)
        {
            return false;
        }
        state.rejected = true;
        true
    }
    pub(crate) fn lobster_recovery_needed(&self) -> bool {
        let state = self.authorization.material.lock();
        state.lobsterai.is_some() && !state.recovery_checked
    }
    pub(crate) fn confirm_lobster_recovery(&self, generation: u64, recorded: bool) {
        let mut state = self.authorization.material.lock();
        if state.active && state.generation == generation && self.is_routing_active() {
            state.recovery_checked = true;
            state.rejected |= recorded;
        }
    }

    pub(super) fn from_credential(cred: &ProviderCredentialYaml) -> Option<Self> {
        Some(Self {
            authorization: Arc::new(refresh_material::RefreshAuthorization::from_credential(
                cred,
            )?),
            routing_active: std::sync::atomic::AtomicBool::new(true),
        })
    }

    pub(super) fn routing_replacement(&self) -> Self {
        Self {
            authorization: self.authorization.clone(),
            routing_active: std::sync::atomic::AtomicBool::new(true),
        }
    }

    pub(crate) fn refresh_endpoint(&self) -> &str {
        &self.authorization.endpoint
    }

    #[cfg(test)]
    pub(crate) fn client_id(&self) -> &str {
        &self.authorization.client_id
    }

    /// Cancellation drops the existing owned guard; policy replacements share this same lock.
    pub(crate) fn try_refresh_lock(&self) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        if !self.is_routing_active() {
            return None;
        }
        self.authorization
            .attempt_lock
            .clone()
            .try_lock_owned()
            .ok()
    }

    pub(crate) fn is_routing_active(&self) -> bool {
        self.routing_active.load(Ordering::Acquire)
    }

    pub(crate) fn dispatch_readiness(&self) -> Result<(), crate::error::GatewayError> {
        let state = self.authorization.material.lock();
        let revoked = !self.is_routing_active() || !state.active;
        // Fresh shared material never revives an old routing wrapper.
        let quarantined = (state.nous_portal.is_some() || state.lobsterai.is_some())
            && state.reauthorization_required;
        if !revoked
            && !quarantined
            && state.recovery_checked
            && !state.rejected
            && state
                .expires_at
                .is_none_or(|at| at > std::time::Instant::now())
        {
            return Ok(());
        }
        let (code, message) = if revoked || state.reauthorization_required {
            (
                "oauth_reauthorization_required",
                "OAuth authorization must be renewed",
            )
        } else {
            (
                "upstream_credential_cooling",
                "OAuth credential refresh is pending",
            )
        };
        Err(crate::error::GatewayError::service_unavailable(message).with_code(code))
    }

    pub(super) fn has_active_refresh_material(&self) -> bool {
        let state = self.authorization.material.lock();
        self.is_routing_active()
            && state.active
            && !self.refresh_endpoint().trim().is_empty()
            && !state.refresh_token.trim().is_empty()
    }

    pub(crate) fn retire(&self) {
        // Serialize retirement with exchange admission; already-issued tickets keep material ownership.
        let _state = self.authorization.material.lock();
        self.routing_active.store(false, Ordering::Release);
    }

    pub(super) fn revoke_authorization(&self) {
        let mut state = self.authorization.material.lock();
        state.active = false;
        state.generation = state.generation.wrapping_add(1);
    }

    pub(crate) fn runtime_override(&self) -> (Option<String>, Option<String>) {
        let state = self.authorization.material.lock();
        (state.api_key_override.clone(), state.expires_at_iso.clone())
    }

    /// One material lock prevents mixing a rotated bearer with the previous inference host.
    pub(crate) fn runtime_dispatch_override(
        &self,
    ) -> (Option<String>, Option<String>, Option<&'static str>) {
        let state = self.authorization.material.lock();
        (
            state.api_key_override.clone(),
            state.expires_at_iso.clone(),
            state
                .nous_portal
                .as_ref()
                .map(|c| c.inference_target().base_url()),
        )
    }

    #[cfg(test)]
    pub(crate) fn refresh_snapshot(&self, margin: std::time::Duration) -> Option<(u64, String)> {
        let state = self.authorization.material.lock();
        if !self.is_routing_active()
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
        Some((state.generation, state.refresh_token.clone()))
    }

    #[cfg(test)]
    pub(crate) fn apply_refresh_if_active(
        &self,
        generation: u64,
        access: String,
        refresh: Option<String>,
        lifetime: u64,
        expiry: String,
    ) -> Option<u64> {
        if !self.is_routing_active() {
            return None;
        }
        self.authorization.apply(
            generation,
            access,
            refresh,
            Some(lifetime),
            Some(expiry),
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn record_refresh_failure(&self, generation: u64, terminal: bool) {
        if self.is_routing_active() {
            self.authorization.record_failure(generation, terminal);
        }
    }

    #[cfg(test)]
    pub(crate) fn is_active(&self) -> bool {
        self.is_routing_active()
    }

    #[cfg(test)]
    pub(crate) fn is_generation_active(&self, generation: u64) -> bool {
        self.is_routing_active() && self.authorization_is_current(self, generation)
    }

    #[cfg(test)]
    pub(crate) fn authorization_context(
        &self,
    ) -> Option<crate::protocol::grok_oauth::AuthorizationContext> {
        self.authorization.material.lock().grok_oauth.clone()
    }

    #[cfg(test)]
    pub(crate) fn force_due(&self) {
        self.authorization.material.lock().expires_at = Some(std::time::Instant::now());
    }
}

impl std::fmt::Debug for TokenRefreshState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenRefreshState")
            .field("endpoint", &self.refresh_endpoint())
            .field("routing_active", &self.is_routing_active())
            .finish_non_exhaustive()
    }
}
