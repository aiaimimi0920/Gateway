//! OAuth refresh deadlines, generation checks and retirement.

use super::*;

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
    pub(super) fn from_credential(cred: &ProviderCredentialYaml) -> Option<Self> {
        let rt = cred
            .refresh_token
            .as_ref()
            .filter(|s| !s.trim().is_empty())?;
        let endpoint = cred
            .refresh_endpoint
            .as_ref()
            .filter(|s| !s.trim().is_empty())?;
        let expiry = cred.expires_at.as_deref().or_else(|| {
            cred.session_auth
                .as_ref()
                .and_then(|s| s.expires_at.as_deref())
        });
        let parsed = expiry.and_then(|s| {
            time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
        });
        // Missing/invalid absolute expiry is unknown, not a fresh lifetime on every restart.
        let remaining = parsed
            .map(|at| {
                (at - time::OffsetDateTime::now_utc())
                    .whole_seconds()
                    .max(0) as u64
            })
            .unwrap_or(0);
        Some(Self {
            refresh_token: parking_lot::Mutex::new(rt.clone()),
            refresh_endpoint: endpoint.clone(),
            client_id: cred.refresh_client_id.clone().unwrap_or_default(),
            expires_in_secs: effective_refresh_lifetime_secs(
                cred.token_expires_in_secs.unwrap_or(21600),
            ),
            expires_at: parking_lot::Mutex::new(safe_refresh_deadline(remaining)),
            expires_at_iso: parking_lot::Mutex::new(parsed.and_then(|at| {
                at.format(&time::format_description::well_known::Rfc3339)
                    .ok()
            })),
            api_key_override: parking_lot::Mutex::new(None),
            attempt_lock: Arc::new(tokio::sync::Mutex::new(())),
            lifecycle: parking_lot::Mutex::new(RefreshLifecycle {
                active: true,
                generation: 0,
                failures: 0,
                retry_at: None,
                reauthorization_required: false,
            }),
        })
    }

    /// The owned guard releases even if shutdown cancels an HTTP refresh future.
    pub(crate) fn try_refresh_lock(&self) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        self.attempt_lock.clone().try_lock_owned().ok()
    }

    /// Expired access material waits for the existing single-flight worker, never starts a second refresh.
    pub(crate) fn dispatch_readiness(&self) -> Result<(), crate::error::GatewayError> {
        let lifecycle = self.lifecycle.lock();
        if *self.expires_at.lock() > std::time::Instant::now() {
            return Ok(());
        }
        let (code, message) = if !lifecycle.active || lifecycle.reauthorization_required {
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

    pub(crate) fn record_refresh_failure(&self, generation: u64, terminal: bool) {
        let mut lifecycle = self.lifecycle.lock();
        if !lifecycle.active || lifecycle.generation != generation {
            return;
        }
        lifecycle.failures = lifecycle.failures.saturating_add(1);
        lifecycle.reauthorization_required = terminal;
        let delay = 60u64.saturating_mul(1 << lifecycle.failures.saturating_sub(1).min(6));
        lifecycle.retry_at = Some(safe_refresh_deadline(delay.min(3600)));
    }

    /// Check configured material without copying, rewriting, or logging the secret.
    /// The lifecycle lock follows the same lock order as refresh/retirement.
    pub(super) fn has_active_refresh_material(&self) -> bool {
        let lifecycle = self.lifecycle.lock();
        lifecycle.active
            && !self.refresh_endpoint.trim().is_empty()
            && !self.refresh_token.lock().trim().is_empty()
    }

    pub(crate) fn refresh_snapshot(
        &self,
        refresh_margin: std::time::Duration,
    ) -> Option<(u64, String)> {
        let lifecycle = self.lifecycle.lock();
        if !lifecycle.active
            || lifecycle.reauthorization_required
            || lifecycle
                .retry_at
                .is_some_and(|at| at > std::time::Instant::now())
        {
            return None;
        }
        let expires_at = *self.expires_at.lock();
        if expires_at.saturating_duration_since(std::time::Instant::now()) > refresh_margin {
            return None;
        }
        let refresh_token = self.refresh_token.lock().clone();
        Some((lifecycle.generation, refresh_token))
    }

    pub(crate) fn apply_refresh_if_active(
        &self,
        generation: u64,
        access_token: String,
        refresh_token: Option<String>,
        expires_in_secs: u64,
        expires_at_iso: String,
    ) -> Option<u64> {
        let mut lifecycle = self.lifecycle.lock();
        if !lifecycle.active || lifecycle.generation != generation {
            return None;
        }

        *self.api_key_override.lock() = Some(access_token);
        if let Some(refresh_token) = refresh_token {
            *self.refresh_token.lock() = refresh_token;
        }
        *self.expires_at.lock() = safe_refresh_deadline(expires_in_secs);
        *self.expires_at_iso.lock() = Some(expires_at_iso);
        lifecycle.generation = lifecycle.generation.wrapping_add(1);
        lifecycle.failures = 0;
        lifecycle.retry_at = None;
        lifecycle.reauthorization_required = false;
        Some(lifecycle.generation)
    }

    pub(crate) fn retire(&self) {
        let mut lifecycle = self.lifecycle.lock();
        lifecycle.active = false;
        lifecycle.generation = lifecycle.generation.wrapping_add(1);
    }

    #[cfg(test)]
    pub(crate) fn is_active(&self) -> bool {
        self.lifecycle.lock().active
    }

    pub(crate) fn is_generation_active(&self, generation: u64) -> bool {
        let lifecycle = self.lifecycle.lock();
        lifecycle.active && lifecycle.generation == generation
    }

    pub(crate) fn runtime_override(&self) -> (Option<String>, Option<String>) {
        let _lifecycle = self.lifecycle.lock();
        (
            self.api_key_override.lock().clone(),
            self.expires_at_iso.lock().clone(),
        )
    }
}

impl std::fmt::Debug for TokenRefreshState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenRefreshState")
            .field("endpoint", &self.refresh_endpoint)
            .field("expires_in_secs", &self.expires_in_secs)
            .finish()
    }
}
