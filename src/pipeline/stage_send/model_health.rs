//! Failed candidates have no winning selection; persist their limit evidence
//! before fallback, without attributing it to a later successful provider.
use super::*;

pub(super) async fn record_rate_limit(
    ctx: &PipelineContext,
    state: &AppState,
    candidate: &crate::routing::candidate::RouteCandidate,
    candidate_index: usize,
    model: &str,
    error: &GatewayError,
) {
    if !should_record_provider_failure(error) {
        return;
    }
    let classification = crate::provider_failure::classify_provider_failure(
        error.http_status,
        error.code.as_deref(),
        Some(&error.message),
    );
    if !matches!(
        classification.class,
        crate::provider_failure::ProviderFailureClass::RateLimited
            | crate::provider_failure::ProviderFailureClass::CredentialInvalid
            | crate::provider_failure::ProviderFailureClass::CredentialExpired
    ) {
        return;
    }
    let input = crate::db::RecordCredentialModelFailureInput {
        provider_account_id: candidate.provider_account_id.clone(),
        provider_credential_id: candidate.provider_credential_id.clone(),
        provider_credential_ref: candidate
            .payload
            .credential_id
            .clone()
            .or_else(|| {
                ctx.projected_access_candidates
                    .get(candidate_index)
                    .map(|row| row.platform_access_id.clone())
            })
            .or_else(|| candidate.provider_credential_id.clone()),
        protocol_profile: Some(candidate.protocol_profile.clone()),
        model: model.to_string(),
        upstream_status: error.http_status,
        upstream_retry_after_ms: error.upstream_retry_after_ms,
        error_message: Some(error.message.clone()),
        classification,
    };
    if let Err(error) = crate::local_runtime::model_states::failure(state, input).await {
        tracing::warn!(req_id = %ctx.req_id, error = %error,
            "failed to persist candidate credential/model failure");
    }
}
