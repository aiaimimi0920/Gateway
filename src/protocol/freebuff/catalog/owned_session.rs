//! Exact-claim, short-lived active observations; display inspection is not execution authority.
use super::{
    business_wire::{self, Authority, Operation},
    BusinessError, BusinessReply, CatalogError, CatalogSession, DeviceRegistration, SessionClaim,
};
use std::time::Duration;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(crate) struct RecoveryRequests<'a> {
    pub(super) authority: Authority<'a>,
}
impl super::BusinessRequests<'_> {
    pub(crate) fn recovery_requests(&self) -> RecoveryRequests<'_> {
        RecoveryRequests {
            authority: self.authority(),
        }
    }
}
impl CatalogSession {
    /// Recovery uses current fetchId but never selects a new model or obtains a new purchase handle.
    pub(crate) fn recovery_requests<'a>(
        &'a self,
        device: &'a DeviceRegistration,
    ) -> Result<RecoveryRequests<'a>, BusinessError> {
        device.check_binding(&self.binding)?;
        let catalog = self.catalog.as_ref().ok_or(CatalogError::CatalogRequired)?;
        Ok(RecoveryRequests {
            authority: Authority {
                binding: &self.binding,
                http: &self.http,
                device,
                fetch_id: catalog.fetch_id(),
            },
        })
    }
}
impl RecoveryRequests<'_> {
    /// Explicit original-attempt release, independent of current model availability. Not a GET,
    /// automatic Drop cleanup, refund guarantee, or permission to prepare another purchase.
    pub(crate) async fn end(
        &self,
        claim: &SessionClaim,
        budget: Duration,
    ) -> Result<BusinessReply, BusinessError> {
        if claim.scope != self.authority.binding.scope {
            return Err(CatalogError::ScopeMismatch.into());
        }
        business_wire::session(self.authority, Operation::End(claim), None, budget).await
    }
    pub(crate) async fn heartbeat(
        &self,
        claim: &SessionClaim,
        budget: Duration,
    ) -> Result<BusinessReply, BusinessError> {
        if claim.scope != self.authority.binding.scope {
            return Err(CatalogError::ScopeMismatch.into());
        }
        business_wire::session(self.authority, Operation::Heartbeat(claim), None, budget).await
    }
}
pub(crate) struct OwnedActiveSession {
    claim: SessionClaim,
    model_key: String,
    expires_at: OffsetDateTime,
    observed_at: std::time::Instant,
    usable_for: Duration,
}
impl std::fmt::Debug for OwnedActiveSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffOwnedActiveSession { identity: <redacted> }")
    }
}
impl OwnedActiveSession {
    /// Only the original claim and stable model may become active. Nothing is inferred from 2xx alone.
    pub(crate) fn parse(
        reply: BusinessReply,
        claim: &SessionClaim,
        model: &str,
        now: OffsetDateTime,
    ) -> Result<Self, BusinessError> {
        let body = &reply.body;
        let text = |field| body.get(field).and_then(serde_json::Value::as_str);
        if reply.status != 200
            || text("status") != Some("active")
            || text("instanceId") != Some(claim.instance_id())
            || text("model") != Some(model)
            || text("accessTier").is_none_or(|s| {
                s.is_empty() || s.len() > 512 || s.trim() != s || s.chars().any(char::is_control)
            })
        {
            return Err(BusinessError::InvalidResponse);
        }
        let instant = |field| {
            text(field)
                .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok())
                .ok_or(BusinessError::InvalidResponse)
        };
        let admitted = instant("admittedAt")?;
        let expires_at = instant("expiresAt")?;
        let remaining = body
            .get("remainingMs")
            .and_then(serde_json::Value::as_f64)
            .filter(|n| n.is_finite() && *n > 0.0)
            .ok_or(BusinessError::InvalidResponse)?;
        // Do not assume synchronized wall clocks for admittedAt. Both expiry and remainingMs
        // cap the monotonic observation lifetime; a future admittedAt cannot extend it.
        if admitted >= expires_at || expires_at <= now {
            return Err(BusinessError::InvalidResponse);
        }
        let usable = remaining.min((expires_at - now).as_seconds_f64() * 1000.0) / 1000.0;
        let usable_for =
            Duration::try_from_secs_f64(usable).map_err(|_| BusinessError::InvalidResponse)?;
        Ok(Self {
            claim: claim.clone(),
            model_key: model.into(),
            expires_at,
            observed_at: std::time::Instant::now(),
            usable_for,
        })
    }
    pub(crate) fn claim(&self) -> &SessionClaim {
        &self.claim
    }
    pub(crate) fn model_key(&self) -> &str {
        &self.model_key
    }
    pub(crate) fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
    pub(crate) fn is_live(&self) -> bool {
        self.observed_at.elapsed() < self.usable_for
    }
}
