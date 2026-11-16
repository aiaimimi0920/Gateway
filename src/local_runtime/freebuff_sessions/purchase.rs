//! Internal request-time quote/claim composition, not an idle management purchase endpoint.
use super::*;
use crate::protocol::freebuff::catalog::{ConfirmedPurchase, PurchaseContext, PurchaseError};
use std::time::Duration;
#[derive(Clone, Copy)]
enum ExistingClaim {
    Restore,
    Reject,
}

fn purchase_error(error: PurchaseError) -> GatewayError {
    GatewayError::conflict("FreeBuff purchase consent is missing, stale, changed or unavailable. No new admission POST was sent; inspect and explicitly confirm a fresh quote.")
        .with_code(error.to_string())
}
async fn restore(
    recovery: SessionRecovery,
    requests: &BusinessRequests<'_>,
    fence: &impl Fn() -> Result<(), GatewayError>,
    budget: Duration,
    policy: ExistingClaim,
) -> Result<ActiveAdmission, GatewayError> {
    if matches!(policy, ExistingClaim::Reject) {
        return Err(GatewayError::conflict(
            "Reconcile the original FreeBuff claim; a public request cannot reuse or replace it.",
        )
        .with_code("freebuff_public_existing_claim"));
    }
    if recovery.model_key() != requests.stable_model_key() {
        return Err(GatewayError::conflict(
            "Reconcile the original FreeBuff model; no replacement session was purchased.",
        )
        .with_code("freebuff_session_model_conflict"));
    }
    fence()?;
    let active = recovery
        .heartbeat(&requests.recovery_requests(), budget)
        .await?;
    fence()?;
    Ok(active)
}
impl LocalRuntime {
    /// Call only for an actual queued internal request; legacy restoration is not public consent.
    /// `fence` must reuse the pinned management Grant/revision authorization, never a stored boolean.
    pub(crate) async fn admit_freebuff_for_request(
        &self,
        context: &PurchaseContext,
        requests: &BusinessRequests<'_>,
        confirmation: Option<ConfirmedPurchase>,
        fence: impl Fn() -> Result<(), GatewayError>,
        budget: Duration,
    ) -> Result<ActiveAdmission, GatewayError> {
        self.admit_request(
            context,
            requests,
            confirmation,
            fence,
            budget,
            ExistingClaim::Restore,
            || async { Ok(()) },
            || Ok(()),
        )
        .await
    }
    pub(crate) async fn admit_freebuff_new_request<
        Fut: std::future::Future<Output = Result<(), GatewayError>>,
    >(
        &self,
        context: &PurchaseContext,
        requests: &BusinessRequests<'_>,
        confirmation: Option<ConfirmedPurchase>,
        fence: impl Fn() -> Result<(), GatewayError>,
        budget: Duration,
        before_send: impl FnOnce() -> Fut,
        on_send: impl FnOnce() -> Result<(), GatewayError>,
    ) -> Result<ActiveAdmission, GatewayError> {
        self.admit_request(
            context,
            requests,
            confirmation,
            fence,
            budget,
            ExistingClaim::Reject,
            before_send,
            on_send,
        )
        .await
    }
    async fn admit_request<Fut: std::future::Future<Output = Result<(), GatewayError>>>(
        &self,
        context: &PurchaseContext,
        requests: &BusinessRequests<'_>,
        confirmation: Option<ConfirmedPurchase>,
        fence: impl Fn() -> Result<(), GatewayError>,
        budget: Duration,
        policy: ExistingClaim,
        before_send: impl FnOnce() -> Fut,
        on_send: impl FnOnce() -> Result<(), GatewayError>,
    ) -> Result<ActiveAdmission, GatewayError> {
        enabled()?;
        if budget.is_zero() {
            return Err(purchase_error(PurchaseError::Business(
                BusinessError::InvalidInput,
            )));
        }
        tokio::time::timeout(budget.min(Duration::from_secs(20)), async {
            fence()?;
            // An existing durable claim wins even over absent/expired consent. GET is not a purchase.
            if let Some(recovery) = self
                .recover_freebuff_admission(
                    &context.provider,
                    &context.credential,
                    &requests.recovery_requests(),
                )
                .await?
            {
                return restore(recovery, requests, &fence, budget, policy).await;
            }
            let confirmation =
                confirmation.ok_or_else(|| purchase_error(PurchaseError::Required))?;
            confirmation
                .check(context, requests)
                .map_err(purchase_error)?;
            fence()?;
            // No SQLite transaction or OS slot guard spans this network read.
            let current = confirmation
                .revalidate(context, requests, budget)
                .await
                .map_err(purchase_error)?;
            let check = || {
                fence()?;
                current.check(context, requests).map_err(purchase_error)
            };
            check()?;
            match self
                .prepare_admission_checked(
                    &context.provider,
                    &context.credential,
                    requests,
                    SessionChargeConsent::MaySpendFreebucks,
                    &check,
                )
                .await?
            {
                AdmissionPreparation::Recover(recovery) => {
                    restore(recovery, requests, &fence, budget, policy).await
                }
                AdmissionPreparation::Send(pending) => {
                    // Revocation/expiry after commit retains the unsent local claim. Never erase/rebuy.
                    before_send().await?;
                    check()?;
                    // Only this unique paid POST consumes an attempt; rejections/restoration do not.
                    on_send()?;
                    let active = pending.send(budget).await?;
                    fence()?;
                    Ok(active)
                }
            }
        })
        .await
        .map_err(|_| GatewayError::service_unavailable(
            "FreeBuff request admission timed out. Any original local claim was retained; do not automatically retry a purchase.",
        ).with_code(BusinessError::Timeout.to_string()))?
    }
}
