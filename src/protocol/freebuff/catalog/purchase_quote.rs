//! Frozen, one-use local purchase consent. No server price ceiling, public receipt or auto-purchase.
use super::{
    inspection::InspectedSession,
    meter::{AvailableMeter, MeterSnapshot},
    BusinessError, BusinessReply, BusinessRequests, ClientSurface, SessionInspection,
};
use serde::Serialize;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
const LIFETIME: Duration = Duration::from_secs(60);

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PurchaseContext {
    pub(crate) provider: String,
    pub(crate) credential: String,
    revision: String,
}
impl PurchaseContext {
    pub(crate) fn new(
        provider: &str,
        credential: &str,
        revision: &str,
    ) -> Result<Self, PurchaseError> {
        if [provider, credential, revision]
            .iter()
            .any(|s| s.is_empty() || s.len() > 512 || s.chars().any(char::is_control))
        {
            return Err(PurchaseError::Mismatch);
        }
        Ok(Self {
            provider: provider.into(),
            credential: credential.into(),
            revision: revision.into(),
        })
    }
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum PurchaseError {
    #[error(transparent)]
    Business(#[from] BusinessError),
    #[error("freebuff_purchase_quote_required")]
    Required,
    #[error("freebuff_purchase_quote_mismatch")]
    Mismatch,
    #[error("freebuff_purchase_quote_changed")]
    Changed,
    #[error("freebuff_purchase_quote_expired")]
    Expired,
    #[error("freebuff_purchase_price_unavailable")]
    Unavailable,
    #[error("freebuff_purchase_insufficient_funds")]
    InsufficientFunds,
    #[error("freebuff_purchase_existing_session")]
    ExistingSession,
}
#[derive(PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PurchaseView {
    model_key: String,
    meter: AvailableMeter,
    wallet_spend: f64,
    claim_earned: bool,
}
// Neither capability is Clone or Deserialize. The view is informational, never proof of consent.
pub(crate) struct PurchaseQuote {
    context: PurchaseContext,
    scope: [u8; 32],
    device: [u8; 32],
    issued: Instant,
    view: PurchaseView,
}
pub(crate) struct ConfirmedPurchase(PurchaseQuote);
pub(crate) struct RevalidatedPurchase {
    confirmation: ConfirmedPurchase,
    latest: BusinessReply,
}
fn fingerprint(requests: &BusinessRequests<'_>) -> [u8; 32] {
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    for value in [
        requests.device.key_id.as_str(),
        &requests.device.key.public_key(),
    ] {
        hash.update(&(value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    hash.finish().as_ref().try_into().expect("SHA-256 size")
}
fn view(
    reply: &BusinessReply,
    model: &str,
    now: OffsetDateTime,
) -> Result<PurchaseView, PurchaseError> {
    let inspection = SessionInspection::parse_ref(reply, model, now)?;
    if !matches!(
        inspection.session,
        InspectedSession::None | InspectedSession::Ended { in_grace: false }
    ) {
        // No foreign-instance takeover, even if its model matches. Recovery has a different owner.
        return Err(PurchaseError::ExistingSession);
    }
    let MeterSnapshot::Available(meter) = inspection.meter else {
        return Err(PurchaseError::Unavailable);
    };
    let price = meter.session_price.ok_or(PurchaseError::Unavailable)?;
    let threshold = (price - meter.solo_discount.unwrap_or(0.0)).max(0.0);
    // Equivalent to balance + grant >= threshold, without overflow of finite f64 inputs.
    if meter.quota_exempt != Some(true)
        && meter.balance < threshold
        && meter.claimable_grant.unwrap_or(0.0) < threshold - meter.balance
    {
        return Err(PurchaseError::InsufficientFunds);
    }
    // Public client estimates, not discounted final billing or a server-enforced wallet ceiling.
    let wallet_spend = (price - meter.daily_remaining).max(0.0);
    let claim_earned = meter.balance < price && meter.quota_exempt != Some(true);
    Ok(PurchaseView {
        model_key: model.into(),
        meter,
        wallet_spend,
        claim_earned,
    })
}
impl BusinessRequests<'_> {
    pub(crate) fn stable_model_key(&self) -> &str {
        self.selected.key
    }
    pub(crate) async fn purchase_quote(
        &self,
        context: &PurchaseContext,
        budget: Duration,
    ) -> Result<PurchaseQuote, PurchaseError> {
        if self.binding.client != ClientSurface::Cli {
            return Err(PurchaseError::Mismatch);
        }
        let reply = self.inspect(budget).await?;
        Ok(PurchaseQuote {
            context: context.clone(),
            scope: self.binding.scope,
            device: fingerprint(self),
            issued: Instant::now(),
            view: view(&reply, self.selected.key, OffsetDateTime::now_utc())?,
        })
    }
}
impl PurchaseQuote {
    /// Queue expiry is anchored to the original inspect, never restarted by listing or approval.
    pub(crate) fn pending_lifetime(
        &self,
        context: &PurchaseContext,
        model: &str,
    ) -> Result<Duration, PurchaseError> {
        if self.context != *context || self.view.model_key != model {
            return Err(PurchaseError::Mismatch);
        }
        LIFETIME
            .checked_sub(self.issued.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(PurchaseError::Expired)
    }
    pub(crate) fn view(&self) -> &PurchaseView {
        &self.view
    }
    /// Only the original request consumes this capability after an explicit immutable-view decision.
    /// Management transports that decision; it never receives or reconstructs this capability.
    pub(crate) fn confirm(self) -> ConfirmedPurchase {
        ConfirmedPurchase(self)
    }
}
impl ConfirmedPurchase {
    pub(crate) fn check(
        &self,
        context: &PurchaseContext,
        requests: &BusinessRequests<'_>,
    ) -> Result<(), PurchaseError> {
        let quote = &self.0;
        if quote.context != *context
            || quote.scope != requests.binding.scope
            || quote.device != fingerprint(requests)
            || quote.view.model_key != requests.selected.key
        {
            return Err(PurchaseError::Mismatch);
        }
        if quote.issued.elapsed() >= LIFETIME {
            return Err(PurchaseError::Expired);
        }
        Ok(())
    }
    pub(crate) async fn revalidate(
        self,
        context: &PurchaseContext,
        requests: &BusinessRequests<'_>,
        budget: Duration,
    ) -> Result<RevalidatedPurchase, PurchaseError> {
        self.check(context, requests)?;
        let latest = requests.inspect(budget).await?;
        let checked = RevalidatedPurchase {
            confirmation: self,
            latest,
        };
        checked.check(context, requests)?;
        Ok(checked)
    }
}
impl RevalidatedPurchase {
    /// Reproject only fresh metadata; NEVER reprice the old displayed quote at the current time.
    /// This catches known scheduled boundaries across commit waits, not concurrent remote changes.
    pub(crate) fn check(
        &self,
        context: &PurchaseContext,
        requests: &BusinessRequests<'_>,
    ) -> Result<(), PurchaseError> {
        self.confirmation.check(context, requests)?;
        let current = view(
            &self.latest,
            requests.selected.key,
            OffsetDateTime::now_utc(),
        )?;
        if current != self.confirmation.0.view {
            return Err(PurchaseError::Changed);
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;
