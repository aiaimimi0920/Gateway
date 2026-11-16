//! Capture only server auth/request state and one exact current candidate; never caller metadata.
use super::failure;
use crate::{
    error::GatewayError,
    pipeline::PipelineContext,
    protocol::freebuff::{business_target, catalog::PurchaseContext},
    routing::{candidate::RouteCandidate, config::RouteConfigSnapshot},
};
use ring::digest::{digest, SHA256};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RequestBinding {
    pub(super) context: PurchaseContext,
    pub(super) request_id: Uuid,
    pub(super) caller: [u8; 32],
    pub(super) body: [u8; 32],
    pub(super) model: String,
}
impl RequestBinding {
    pub(crate) fn context(&self) -> &PurchaseContext {
        &self.context
    }
    /// Capture again from the current request and actual final harness bytes before consumption.
    /// `snapshot` must be the quote-time snapshot, not the potentially older initial route snapshot.
    pub(crate) fn capture(
        ctx: &PipelineContext,
        snapshot: &RouteConfigSnapshot,
        candidate: &RouteCandidate,
        model: &str,
        final_body: &[u8],
    ) -> Result<Self, GatewayError> {
        let invalid = || failure("freebuff_purchase_request_binding_invalid");
        let session = ctx.session.as_ref().ok_or_else(invalid)?;
        let concrete_api = session
            .api_key_id
            .as_deref()
            .filter(|id| !crate::auth::session::is_synthetic_api_key_id(id) && !id.is_empty());
        if concrete_api.is_none()
            && session.user_credential_id.is_none()
            && session.access_key_id.is_none()
        {
            return Err(failure("freebuff_purchase_caller_required"));
        }
        let credential = candidate
            .payload
            .credential_id
            .as_deref()
            .ok_or_else(invalid)?;
        // Exact lookup must not advance the pool's round-robin cursor or silently pick another
        // account. Model eligibility still belongs to the actual stage_send candidate owner.
        let expected = snapshot
            .select_credential_probe_target(credential)
            .ok_or_else(invalid)?;
        // A recovery refresh must not smuggle a different payload under the old revision/candidate.
        if !expected.enabled
            || expected.provider_id != candidate.provider_account_id
            || candidate.provider_credential_id.is_some()
            || serde_json::to_value(&expected.payload).map_err(|_| invalid())?
                != serde_json::to_value(&candidate.payload).map_err(|_| invalid())?
            || business_target::read(candidate.payload.extra_body.as_ref())?.is_none()
            || candidate.payload.canonical_adapter() != "freebuff_compatible"
            || final_body.is_empty()
            || final_body.len() > 50 * 1024 * 1024
        {
            return Err(invalid());
        }
        let context = PurchaseContext::new(
            &candidate.provider_account_id,
            credential,
            snapshot.revision().id(),
        )
        .map_err(|_| invalid())?;
        // Fixed tuple: excludes user_id/body.user, x-request-id, arbitrary headers and raw bearer secrets.
        let principal = serde_json::to_vec(&(
            &session.tenant_id,
            &session.project_id,
            concrete_api,
            &session.user_credential_id,
            &session.access_key_id,
            &session.credential_ref,
            &ctx.requesting_access_key_id,
            &ctx.source_access_key_id,
            ctx.canonical_req.endpoint_kind,
            ctx.canonical_req.stream,
        ))
        .map_err(|_| invalid())?;
        Ok(Self {
            context,
            request_id: ctx.req_id,
            caller: digest(&SHA256, &principal)
                .as_ref()
                .try_into()
                .expect("SHA-256 size"),
            body: digest(&SHA256, final_body)
                .as_ref()
                .try_into()
                .expect("SHA-256 size"),
            model: model.into(),
        })
    }
}
