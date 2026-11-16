//! Current catalog/device protocol. This does not buy sessions or replace the legacy executor.
//! Fixed-source evidence and the external management/persistence owner boundaries: catalog/REFERENCE.md.
mod account;
mod business;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod business_safety_tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod business_tests;
mod business_wire;
mod claim_receipt;
pub(crate) use claim_receipt::StoredClaimScope;
mod owned_session;
pub(crate) use owned_session::{OwnedActiveSession, RecoveryRequests};
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod claim_owner_tests;
mod completion;
mod completion_decode;
pub(crate) use completion_decode::resolve_index as resolve_stream_tool_index;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod completion_decode_tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod completion_tests;
mod device;
mod execution;
mod finish_wire;
pub(crate) use execution::CatalogRun;
mod inspection;
mod meter;
mod model;
mod model_policy;
mod relay_step;
mod root_builder;
mod root_context;
mod root_history;
mod root_summary;
pub(crate) use execution::root_turn::RootTurn;
pub(crate) use relay_step::RelayStep;
pub(crate) use root_history::RootHistory;
mod root_model;
pub use inspection::SessionInspection;
pub(crate) use model_policy::RootCompactionPolicy;
pub(crate) use root_model::RootModelInput;
#[cfg(test)]
mod inspection_tests;
mod persistence;
mod purchase_quote;
pub(crate) use purchase_quote::{ConfirmedPurchase, PurchaseContext, PurchaseError, PurchaseQuote};
pub(crate) mod run_journal;
mod run_wire;
mod session_end;
pub(crate) use session_end::SessionEndReceipt;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod session_end_tests;
mod transport;
use account::Binding;
pub use account::{BusinessOrigin, ClientSurface};
pub use business::{
    AdmissionRequest, BusinessError, BusinessReply, BusinessRequests, SessionChargeConsent,
    SessionClaim,
};
pub use completion::PreparedCompletion;
pub use device::{DeviceKey, DeviceRegistration};
pub use model::{CatalogBadge, DataUse, ModelAccess, ModelCatalog, ModelRow, SelectedModel};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    #[error("freebuff_catalog_invalid_input")]
    InvalidInput,
    #[error("freebuff_catalog_invalid_key")]
    InvalidKey,
    #[error("freebuff_catalog_invalid_response")]
    InvalidResponse,
    #[error("freebuff_catalog_model_unavailable")]
    ModelUnavailable,
    #[error("freebuff_catalog_required_before_registration")]
    CatalogRequired,
    #[error("freebuff_catalog_registration_scope_mismatch")]
    ScopeMismatch,
    #[error("freebuff_catalog_unsupported")]
    Unsupported,
    #[error("freebuff_catalog_unauthorized")]
    Unauthorized,
    #[error("freebuff_catalog_rejected")]
    Rejected,
    #[error("freebuff_catalog_retryable")]
    Retryable,
    #[error("freebuff_catalog_device_unknown")]
    DeviceUnknown,
    #[error("freebuff_catalog_transport_failed")]
    Transport,
    #[error("freebuff_catalog_timeout")]
    Timeout,
    #[error("freebuff_catalog_redirect_rejected")]
    RedirectRejected,
    #[error("freebuff_catalog_response_too_large")]
    ResponseTooLarge,
    #[error("freebuff_catalog_feature_disabled")]
    FeatureDisabled,
}

/// One immutable account/target/client binding. Refresh and registration are explicit, cancellable steps.
/// The caller owns scheduling and durable keys; this owner has no background tasks or secret store.
pub struct CatalogSession {
    binding: Binding,
    http: transport::CatalogTransport,
    catalog: Option<ModelCatalog>,
}
impl CatalogSession {
    pub fn new(
        origin: BusinessOrigin,
        token: &str,
        client: ClientSurface,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            binding: Binding::new(origin, token, client)?,
            http: transport::CatalogTransport::new()?,
            catalog: None,
        })
    }
    pub fn catalog(&self) -> Option<&ModelCatalog> {
        self.catalog.as_ref()
    }
    /// No device means unsigned bootstrap, not anonymous access. An old bound registration can sign it.
    pub async fn refresh(
        &mut self,
        device: Option<&DeviceRegistration>,
        budget: Duration,
    ) -> Result<&ModelCatalog, CatalogError> {
        // Reject a foreign capability before any network I/O or cache mutation.
        if let Some(device) = device {
            device.check_binding(&self.binding)?;
        }
        let fetched = async {
            let reply = transport::exchange(
                &self.http,
                &self.binding,
                transport::Request::Catalog { device },
                budget,
            )
            .await?;
            reply.success()?;
            ModelCatalog::parse(&reply.bytes)
        }
        .await;
        match fetched {
            Ok(catalog) => self.catalog = Some(catalog),
            Err(error) => {
                // Only temporary network/server failures retain a previously valid snapshot.
                if !matches!(
                    error,
                    CatalogError::Retryable
                        | CatalogError::Transport
                        | CatalogError::Timeout
                        | CatalogError::FeatureDisabled
                        | CatalogError::InvalidInput
                ) {
                    self.catalog = None;
                }
                return Err(error);
            }
        }
        Ok(self
            .catalog
            .as_ref()
            .expect("a successful refresh installed a catalog"))
    }
    /// A public-key registration is not login or admission. Never generated, retried or persisted here.
    pub async fn register_device(
        &self,
        key: Arc<DeviceKey>,
        budget: Duration,
    ) -> Result<DeviceRegistration, CatalogError> {
        self.catalog.as_ref().ok_or(CatalogError::CatalogRequired)?;
        let reply = transport::exchange(
            &self.http,
            &self.binding,
            transport::Request::Register {
                public_key: key.public_key(),
            },
            budget,
        )
        .await?;
        reply.success()?;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Registered {
            key_id: String,
        }
        let response: Registered =
            serde_json::from_slice(&reply.bytes).map_err(|_| CatalogError::InvalidResponse)?;
        if !atom(&response.key_id, 4096) {
            return Err(CatalogError::InvalidResponse);
        }
        Ok(DeviceRegistration::new(&self.binding, key, response.key_id))
    }
}
impl std::fmt::Debug for CatalogSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffCatalogSession { material: <redacted> }")
    }
}
fn atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
tokio::task_local! { static TEST_ORIGIN: String; }
/// Test-only loopback scoping; production never accepts a caller-supplied URL.
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
pub(crate) async fn with_test_origin<F: std::future::Future>(origin: String, test: F) -> F::Output {
    TEST_ORIGIN.scope(origin, test).await
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
pub(crate) fn current_test_origin() -> Option<String> {
    TEST_ORIGIN.try_with(Clone::clone).ok()
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
tokio::task_local! { pub(crate) static TEST_BODY_STARTED: Arc<tokio::sync::Notify>; }
#[cfg(test)]
mod device_tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod fixture;
#[cfg(test)]
mod model_policy_tests;
#[cfg(test)]
mod model_tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod root_model_wire_tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod safety_tests;
#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod wire_tests;
