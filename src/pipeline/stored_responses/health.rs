//! Consume evidence inside the bounded durable owner, after required receipt/ledger writes.
use super::*;
use crate::protocol::sap_aicore::{errors, responses_lifecycle::evidence::Evidence, PROFILE};

pub(super) struct Observed {
    pub target: PinnedTarget,
    pub evidence: Evidence,
}

pub(super) async fn persist(
    state: &AppState,
    started: std::time::Instant,
    observed: Option<Observed>,
) {
    let Some(Observed { target, evidence }) = observed else {
        return;
    };
    let identity = crate::db::RecordCredentialModelSuccessInput {
        provider_account_id: target.provider,
        provider_credential_id: state.pg_pool.as_ref().map(|_| target.credential.clone()),
        provider_credential_ref: Some(target.credential),
        protocol_profile: Some(PROFILE.into()),
        model: target.upstream_model,
    };
    let result = match evidence {
        Evidence::Completed(_) => {
            crate::local_runtime::model_states::success(state, identity).await
        }
        Evidence::Failed { error, status } if !errors::neutral(&error) => {
            let input = crate::db::RecordCredentialModelFailureInput {
                provider_account_id: identity.provider_account_id,
                provider_credential_id: identity.provider_credential_id,
                provider_credential_ref: identity.provider_credential_ref,
                protocol_profile: identity.protocol_profile,
                model: identity.model,
                upstream_status: status,
                upstream_retry_after_ms: error.upstream_retry_after_ms,
                error_message: Some(error.message.clone()),
                classification: crate::provider_failure::classify_provider_failure(
                    error.http_status,
                    error.code.as_deref(),
                    Some(&error.message),
                ),
            };
            crate::routing::model_circuit::health_control::record_failure(
                state,
                input,
                Some(started),
            )
            .await
        }
        // Pending does not clear persistent failure; cleanup success is not generation.
        _ => return,
    };
    if let Err(error) = result {
        tracing::warn!(code = ?error.code, "cannot persist SAP response health evidence");
    }
}
