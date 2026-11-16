//! Frozen SDK identity, not send admission: the attempt gate still owns eligibility and budget.
use super::{CredentialSource, PipelineContext};
use crate::{
    error::{FallbackHint, GatewayError},
    protocol::{canonical::EndpointKind, copilot::TextRequest},
    routing::{candidate::RouteCandidate, config::RouteConfigSnapshot},
    state::AppState,
};
use std::sync::Arc;

// No Debug/Serialize: the frozen route snapshot contains private credential material.
pub(super) struct CopilotIdentity<'a> {
    state: &'a AppState,
    context: &'a PipelineContext,
    candidate: &'a RouteCandidate,
    snapshot: Arc<RouteConfigSnapshot>,
    local_key: &'a str,
    credential: &'a str,
    text: TextRequest<'a>,
}

impl<'a> CopilotIdentity<'a> {
    /// Only trusted pipeline owners supply these values; no caller metadata/header is consulted.
    pub(super) fn bind(
        state: &'a AppState,
        context: &'a PipelineContext,
        candidate: &'a RouteCandidate,
        model: &'a str,
    ) -> Result<Self, GatewayError> {
        let text = crate::protocol::copilot::single_turn(&context.canonical_req, model)?;
        let local_key =
            crate::local_runtime::access_keys::local_key_id(state, context.session.as_ref())
                .ok_or_else(unavailable)?;
        if state.pg_pool.is_some()
            || context.credential_source != CredentialSource::Platform
            || candidate.provider_credential_id.is_some()
        {
            return Err(unavailable());
        }
        let identity = Self {
            state,
            context,
            candidate,
            snapshot: context
                .cash_route_snapshot
                .clone()
                .ok_or_else(unavailable)?,
            local_key,
            credential: candidate
                .payload
                .credential_id
                .as_deref()
                .ok_or_else(unavailable)?,
            text,
        };
        identity.check_lifetime()?;
        identity.saved_token()?;
        Ok(identity)
    }

    pub(super) fn request(&self) -> &TextRequest<'a> {
        &self.text
    }

    /// Reuse at startup and immediately before session creation after any startup await.
    /// This is a point-in-time fence, not a lock across the remote RPC or permission to replay.
    pub(super) async fn token_for_send(&self) -> Result<&str, GatewayError> {
        self.check_lifetime()?;
        let local = self.state.local_runtime.as_ref().ok_or_else(unavailable)?;
        let mut candidates = vec![self.candidate.clone()];
        let (_, metadata) = local
            .authorize_local_candidates_with_metadata(
                self.local_key,
                self.context.canonical_req.requested_model.as_deref(),
                &mut candidates,
            )
            .await
            .map_err(abort)?;
        let scopes = crate::db::scope_list_from_metadata(metadata.as_ref());
        let endpoint = match self.context.canonical_req.endpoint_kind {
            EndpointKind::ChatCompletions => "chat.completions",
            EndpointKind::Responses => "responses",
            _ => return Err(unavailable()),
        };
        if !scopes
            .iter()
            .any(|scope| scope == "relay" || scope == endpoint)
        {
            return Err(unavailable());
        }
        let mut groups = self
            .snapshot
            .account_group_constraint(self.context.account_group_id.as_deref())
            .map_err(|_| unavailable())?;
        if let Some(ids) = crate::access_key_groups::group_ids(metadata.as_ref()).map_err(abort)? {
            groups.intersect(&self.snapshot.access_key_group_constraint(&ids));
        }
        if !groups.allows_candidate(self.candidate) {
            return Err(unavailable());
        }
        // Local policy reads await storage; never publish an identity from a replaced snapshot.
        self.check_lifetime()?;
        self.saved_token()
    }

    fn check_lifetime(&self) -> Result<(), GatewayError> {
        if self.state.shutdown.is_requested()
            || self.state.lifecycle.is_draining()
            || self.state.route_config.snapshot().revision() != self.snapshot.revision()
            || self
                .context
                .caller_cancel
                .as_ref()
                .is_some_and(|rx| *rx.borrow())
        {
            return Err(unavailable());
        }
        self.context.request_budget.check_inflight().map_err(abort)
    }

    fn saved_token(&self) -> Result<&str, GatewayError> {
        let provider = self
            .snapshot
            .document()
            .providers
            .iter()
            .find(|provider| {
                provider.id == self.candidate.provider_account_id
                    && crate::routing::config::copilot::is_provider(provider)
            })
            .ok_or_else(unavailable)?;
        let credential = provider
            .credentials
            .iter()
            .enumerate()
            .find(|(index, credential)| {
                crate::routing::config::effective_credential_id(&provider.id, *index, credential)
                    == self.credential
            })
            .map(|(_, credential)| credential)
            .ok_or_else(unavailable)?;
        if credential.enabled == Some(false) {
            return Err(unavailable());
        }
        if let Some(expiry) = &credential.expires_at {
            let expiry =
                time::OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)
                    .map_err(|_| unavailable())?;
            if expiry <= time::OffsetDateTime::now_utc() {
                return Err(unavailable());
            }
        }
        credential
            .copilot_github_token
            .as_deref()
            .ok_or_else(unavailable)
    }
}

fn abort(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Copilot identity authorization must not be retried or substituted.".into(),
    };
    error
}

fn unavailable() -> GatewayError {
    abort(
        GatewayError::service_unavailable("Copilot execution identity is unavailable")
            .with_code("copilot_identity_unavailable"),
    )
}

#[cfg(test)]
mod tests;
