//! Explicit, catalog-bound wire operations. No automatic purchase, retry, registration or task owner.
//! Management must confirm a current price/balance and retain the claim BEFORE sending admission.
use super::{
    account::Binding, business_wire, transport::CatalogTransport, CatalogError, CatalogSession,
    ClientSurface, DeviceRegistration, SelectedModel,
};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BusinessError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("freebuff_business_invalid_input")]
    InvalidInput,
    #[error("freebuff_business_request_too_large")]
    RequestTooLarge,
    #[error("freebuff_business_transport_failed")]
    Transport,
    #[error("freebuff_business_timeout")]
    Timeout,
    #[error("freebuff_business_cancelled")]
    Cancelled,
    #[error("freebuff_business_redirect_rejected")]
    RedirectRejected,
    #[error("freebuff_business_response_too_large")]
    ResponseTooLarge,
    #[error("freebuff_business_invalid_response")]
    InvalidResponse,
    #[error("freebuff_business_session_inactive")]
    SessionInactive,
    #[error("freebuff_business_run_unconfirmed")]
    RunUnconfirmed,
    #[error("freebuff_business_accounting_unknown")]
    AccountingUnknown,
    #[error("freebuff_business_run_journal_unavailable")]
    RunJournalUnavailable,
    #[error("freebuff_business_context_budget_exceeded")]
    ContextBudgetExceeded,
    #[error("freebuff_business_summary_invalid")]
    SummaryInvalid,
    #[error("freebuff_business_summary_output_limit")]
    SummaryOutputLimit,
}

/// Raw, bounded protocol data, NOT a public DTO or proof that admission succeeded.
/// Typed session/fee owners must interpret HTTP status AND body, not merely a successful transport.
pub struct BusinessReply {
    pub status: u16,
    pub body: Value,
}
impl std::fmt::Debug for BusinessReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreeBuffBusinessReply")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// A marker of the caller's explicit decision, NOT a server-enforced monetary ceiling.
/// Catalog selection/configuration alone must never construct this at a public request boundary.
pub enum SessionChargeConsent {
    MaySpendFreebucks,
}

/// Host-generated CLI execution identity; no caller-provided header/string constructor or serde.
#[derive(Clone)]
pub struct SessionClaim {
    pub(super) scope: [u8; 32],
    pub(super) instance: String,
    pub(super) attempt: String,
}
impl SessionClaim {
    pub fn instance_id(&self) -> &str {
        &self.instance
    }
    pub fn attempt_id(&self) -> &str {
        &self.attempt
    }
}
impl std::fmt::Debug for SessionClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffSessionClaim { identity: <redacted> }")
    }
}

/// Borrowing the immutable account and catalog pins handle and fetchId to the same snapshot.
pub struct BusinessRequests<'a> {
    pub(super) binding: &'a Binding,
    pub(super) http: &'a CatalogTransport,
    pub(super) device: &'a DeviceRegistration,
    pub(super) selected: SelectedModel<'a>,
}
impl CatalogSession {
    pub fn business_requests<'a>(
        &'a self,
        device: &'a DeviceRegistration,
        key: &str,
    ) -> Result<BusinessRequests<'a>, BusinessError> {
        device.check_binding(&self.binding)?;
        let catalog = self.catalog.as_ref().ok_or(CatalogError::CatalogRequired)?;
        Ok(BusinessRequests {
            binding: &self.binding,
            http: &self.http,
            device,
            selected: catalog.select(key)?,
        })
    }
}
impl BusinessRequests<'_> {
    /// Borrow data from this exact catalog selection, not authority to execute a root or buy a session.
    pub(crate) fn root_model_input(&self) -> super::RootModelInput<'_> {
        self.selected.root_input()
    }
    pub(super) fn authority(&self) -> business_wire::Authority<'_> {
        business_wire::Authority {
            binding: self.binding,
            http: self.http,
            device: self.device,
            fetch_id: self.selected.fetch_id,
        }
    }
    /// Full pre-join snapshot, no liveness write and no admission. Never schedules a poll loop.
    pub async fn inspect(&self, budget: Duration) -> Result<BusinessReply, BusinessError> {
        business_wire::session(
            self.authority(),
            business_wire::Operation::Inspect,
            None,
            budget,
        )
        .await
    }
    /// A full per-claim GET is a heartbeat (it may update remote liveness), not a purchase.
    pub async fn heartbeat(
        &self,
        claim: &SessionClaim,
        budget: Duration,
    ) -> Result<BusinessReply, BusinessError> {
        self.check_claim(claim)?;
        business_wire::session(
            self.authority(),
            business_wire::Operation::Heartbeat(claim),
            None,
            budget,
        )
        .await
    }
    /// Explicit exact-attempt cancellation/end. A response is not proof of a refund.
    pub async fn end(
        &self,
        claim: &SessionClaim,
        budget: Duration,
    ) -> Result<BusinessReply, BusinessError> {
        self.check_claim(claim)?;
        business_wire::session(
            self.authority(),
            business_wire::Operation::End(claim),
            None,
            budget,
        )
        .await
    }
    pub(super) fn check_claim(&self, claim: &SessionClaim) -> Result<(), BusinessError> {
        if claim.scope != self.binding.scope {
            return Err(CatalogError::ScopeMismatch.into());
        }
        Ok(())
    }
    /// Preparing is local only. The explicit send consumes this request; no replay API is exposed.
    /// LocalRuntime's admission owner can retain this claim durably; price/balance consent is still external.
    pub fn prepare_cli_admission(
        &self,
        consent: SessionChargeConsent,
    ) -> Result<AdmissionRequest<'_>, BusinessError> {
        let SessionChargeConsent::MaySpendFreebucks = consent;
        // Desktop's purchase claim lifecycle is not inferred from CLI's prefix/protocol.
        if self.binding.client != ClientSurface::Cli {
            return Err(CatalogError::Unsupported.into());
        }
        let attempt = uuid::Uuid::new_v4().to_string();
        let claim = SessionClaim {
            scope: self.binding.scope,
            instance: format!("cli:{attempt}"),
            attempt,
        };
        Ok(AdmissionRequest {
            requests: self,
            claim,
        })
    }
}

/// One explicit POST. A timeout, cancellation, malformed answer or transport error may have charged.
/// The caller must retain claim() before send and reconcile it, never automatically prepare a new POST.
pub struct AdmissionRequest<'a> {
    requests: &'a BusinessRequests<'a>,
    claim: SessionClaim,
}
impl AdmissionRequest<'_> {
    pub fn claim(&self) -> &SessionClaim {
        &self.claim
    }
    pub async fn send(self, budget: Duration) -> Result<BusinessReply, BusinessError> {
        business_wire::session(
            self.requests.authority(),
            business_wire::Operation::Admit(&self.claim),
            Some(self.requests.selected.handle),
            budget,
        )
        .await
    }
}

#[cfg(all(test, not(feature = "line-freebuff-web-reverse-api")))]
mod feature_tests {
    use super::*;
    #[tokio::test]
    async fn freebuff_business_operations_are_explicitly_disabled_without_the_line() {
        let mut catalog = CatalogSession::new(
            super::super::BusinessOrigin::Codebuff,
            "synthetic-token",
            ClientSurface::Cli,
        )
        .unwrap();
        catalog.catalog = Some(
            super::super::ModelCatalog::parse(
                &serde_json::to_vec(&super::super::tests::body()).unwrap(),
            )
            .unwrap(),
        );
        let (private, public) = super::super::tests::pair();
        let key =
            std::sync::Arc::new(super::super::DeviceKey::from_pkcs8(&private, &public).unwrap());
        let device = DeviceRegistration::new(&catalog.binding, key, "device".into());
        let scope = catalog.business_requests(&device, "open").unwrap();
        let disabled = Some(BusinessError::Catalog(CatalogError::FeatureDisabled));
        assert_eq!(scope.inspect(Duration::from_secs(1)).await.err(), disabled);
        let admission = scope
            .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
            .unwrap();
        let claim = admission.claim().clone();
        assert_eq!(admission.send(Duration::from_secs(1)).await.err(), disabled);
        assert_eq!(
            scope.heartbeat(&claim, Duration::from_secs(1)).await.err(),
            disabled
        );
        assert_eq!(
            scope.end(&claim, Duration::from_secs(1)).await.err(),
            disabled
        );
        assert_eq!(
            scope
                .prepare_completion(&serde_json::json!({"model":"fbm1.synthetic-open"}), 32768)
                .unwrap()
                .send(Duration::from_secs(1))
                .await
                .err(),
            disabled
        );
        let root = std::env::temp_dir().join(format!(
            "gateway-local-freebuff-disabled-{}",
            uuid::Uuid::new_v4()
        ));
        let local = crate::local_runtime::LocalRuntime::open(&root)
            .await
            .unwrap();
        assert_eq!(
            local
                .prepare_freebuff_admission(
                    "pool",
                    "account",
                    &scope,
                    SessionChargeConsent::MaySpendFreebucks
                )
                .await
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert_eq!(
            local
                .recover_freebuff_admission(
                    "pool",
                    "account",
                    &catalog.recovery_requests(&device).unwrap()
                )
                .await
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
            .fetch_one(&local.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "disabled line creates no purchase claim");
        local.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }
}
