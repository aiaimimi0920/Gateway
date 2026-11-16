//! The original request owns quote, wait, approval and admission. Drop never buys or replays.
use super::{failure, Decision, Fence, Lease, RequestBinding};
use crate::{
    error::GatewayError,
    local_runtime::{freebuff_sessions::ActiveAdmission, LocalRuntime},
    protocol::freebuff::catalog::{BusinessRequests, PurchaseQuote},
};
use std::time::Duration;
use tokio::{sync::oneshot, time::Instant};
pub(crate) struct WaitingRequest {
    pub(super) lease: Lease,
    pub(super) binding: RequestBinding,
    pub(super) quote: PurchaseQuote,
    pub(super) receiver: oneshot::Receiver<Decision>,
    pub(super) deadline: Instant,
}
pub(crate) struct ApprovedRequest {
    lease: Lease,
    binding: RequestBinding,
    quote: PurchaseQuote,
    fence: Fence,
}
impl WaitingRequest {
    pub(crate) async fn wait(self) -> Result<ApprovedRequest, GatewayError> {
        let Self {
            lease,
            binding,
            quote,
            receiver,
            deadline,
        } = self;
        let decision = tokio::time::timeout_at(deadline, receiver)
            .await
            .map_err(|_| failure("freebuff_purchase_quote_expired"))?
            .map_err(|_| failure("freebuff_purchase_pending_gone"))?;
        let Decision::Approve(fence) = decision else {
            return Err(failure("freebuff_purchase_cancelled"));
        };
        lease.check()?;
        fence()?;
        quote
            .pending_lifetime(&binding.context, &binding.model)
            .map_err(|e| failure(&e.to_string()))?;
        Ok(ApprovedRequest {
            lease,
            binding,
            quote,
            fence,
        })
    }
}
impl ApprovedRequest {
    pub(crate) fn check(&self, current: &RequestBinding) -> Result<(), GatewayError> {
        if self.binding != *current {
            return Err(failure("freebuff_purchase_request_binding_mismatch"));
        }
        self.lease.check()?;
        (self.fence)()
    }
    /// Public callers may buy only a new exact claim, never inherit another request's paid session.
    pub(crate) async fn admit_new<Fut: std::future::Future<Output = Result<(), GatewayError>>>(
        self,
        current: &RequestBinding,
        local: &LocalRuntime,
        requests: &BusinessRequests<'_>,
        budget: Duration,
        before_send: impl FnOnce() -> Fut,
        on_send: impl FnOnce() -> Result<(), GatewayError>,
    ) -> Result<(ActiveAdmission, Fence), GatewayError> {
        self.check(current)?;
        let Self {
            lease,
            binding,
            quote,
            fence,
        } = self;
        let check = || {
            lease.check()?;
            fence()
        };
        let active = local
            .admit_freebuff_new_request(
                &binding.context,
                requests,
                Some(quote.confirm()),
                check,
                budget,
                before_send,
                on_send,
            )
            .await?;
        Ok((active, fence))
    }
    /// The caller must recapture current binding from the same live request/final bytes, not echo a DTO.
    /// Retain the slot and dynamic authority through every existing pre/post-commit and POST fence.
    pub(crate) async fn admit(
        self,
        current: &RequestBinding,
        local: &LocalRuntime,
        requests: &BusinessRequests<'_>,
        budget: Duration,
    ) -> Result<ActiveAdmission, GatewayError> {
        let Self {
            lease,
            binding,
            quote,
            fence,
        } = self;
        if binding != *current {
            return Err(failure("freebuff_purchase_request_binding_mismatch"));
        }
        let check = || {
            lease.check()?;
            fence()
        };
        check()?;
        local
            .admit_freebuff_for_request(
                &binding.context,
                requests,
                Some(quote.confirm()),
                check,
                budget,
            )
            .await
    }
}
