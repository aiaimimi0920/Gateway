//! Pending storage owns the single-flight guard through receipt acknowledgement and Result publication.
use super::{nous, persistence::authority_budget, PendingRefresh};
use crate::{
    console::{refresh_grants::*, RouteConfigRuntime},
    routing::config::*,
};
use std::sync::Arc;

pub(super) struct PendingReceipt {
    owner: Arc<TokenRefreshState>,
    attempt: RefreshAttempt,
    original: ProviderCredentialYaml,
    request: GrantRequest,
    record: GrantRecord,
    receipt: GrantReceipt,
}
pub(super) enum PendingWrite {
    Receipt(Box<PendingReceipt>),
    Result(Box<PendingRefresh>),
    Complete {
        provider: String,
        credential: String,
    },
}
impl PendingWrite {
    pub(super) fn received_lobster(
        owner: Arc<TokenRefreshState>,
        attempt: RefreshAttempt,
        original: ProviderCredentialYaml,
        request: GrantRequest,
        record: GrantRecord,
        tokens: crate::protocol::lobsterai::ReceivedTokens,
    ) -> Self {
        Self::Receipt(Box::new(PendingReceipt {
            owner,
            attempt,
            original,
            request,
            record,
            receipt: GrantReceipt::LobsterAi { tokens },
        }))
    }

    pub(super) fn received(
        owner: Arc<TokenRefreshState>,
        attempt: RefreshAttempt,
        original: ProviderCredentialYaml,
        request: GrantRequest,
        record: GrantRecord,
        tokens: crate::protocol::nous_portal::ReceivedTokens,
    ) -> Self {
        Self::Receipt(Box::new(PendingReceipt {
            owner,
            attempt,
            original,
            request,
            record,
            receipt: GrantReceipt::NousPortal { tokens },
        }))
    }
    pub(super) fn identity(&self) -> (&str, &str) {
        match self {
            Self::Receipt(p) => (&p.request.provider, &p.request.credential),
            Self::Result(p) => p.identity(),
            Self::Complete {
                provider,
                credential,
            } => (provider, credential),
        }
    }
    pub(super) async fn write_to(
        &mut self,
        runtime: Option<&RouteConfigRuntime>,
        redis: Option<&deadpool_redis::Pool>,
    ) -> bool {
        if let Self::Receipt(p) = self {
            let Some(runtime) = runtime else { return false };
            match tokio::time::timeout(
                authority_budget(),
                runtime.store_refresh_grant_receipt(&p.record, &p.receipt),
            )
            .await
            {
                Ok(Ok(record)) => p.record = record,
                _ => {
                    tracing::warn!(provider=%p.request.provider,cred_id=%p.request.credential,
                        "OAuth raw receipt storage unavailable or unacknowledged; retaining successor");
                    return false;
                }
            }
            let admitted = match &p.receipt {
                GrantReceipt::LumaOauth { .. } => Err(super::transport::RefreshError {
                    code: "luma_refresh_publication_not_supported",
                    terminal: true,
                }),
                GrantReceipt::NousPortal { tokens } => nous::admit(&p.request, tokens),
                GrantReceipt::LobsterAi { tokens } => super::lobsterai::admit(&p.request, tokens),
            };
            if let Err(error) = &admitted {
                // The consumed successor remains durable but cannot authorize inference or Result.
                p.attempt.record_failure(true);
                tracing::warn!(provider=%p.request.provider,cred_id=%p.request.credential,code=error.code,
                    "OAuth receipt retained in quarantine; reauthorization required");
            }
            let complete = Self::Complete {
                provider: p.request.provider.clone(),
                credential: p.request.credential.clone(),
            };
            // No await between taking the receipt owner and promoting its exact attempt.
            if let Self::Receipt(p) = std::mem::replace(self, complete) {
                if let Ok(token) = admitted {
                    let pending = if token.lobsterai.is_some() {
                        PendingRefresh::received_unknown(
                            p.owner, p.attempt, p.original, p.request, p.record, token,
                        )
                    } else {
                        let expiry = token
                            .absolute_expiry
                            .clone()
                            .expect("Nous admission supplies absolute expiry");
                        PendingRefresh::received(
                            p.owner, p.attempt, p.original, p.request, p.record, token, expiry,
                        )
                    };
                    *self = Self::Result(Box::new(pending));
                }
            }
        }
        match self {
            Self::Result(p) => p.write_to(runtime, redis).await,
            Self::Complete { .. } => true,
            Self::Receipt(_) => {
                unreachable!("receipt must be retained or promoted before Result write")
            }
        }
    }
    #[cfg(test)]
    pub(super) fn as_result(&self) -> Option<&PendingRefresh> {
        match self {
            Self::Result(p) => Some(p),
            _ => None,
        }
    }
    #[cfg(test)]
    pub(super) fn as_result_mut(&mut self) -> Option<&mut PendingRefresh> {
        match self {
            Self::Result(p) => Some(p),
            _ => None,
        }
    }
}
