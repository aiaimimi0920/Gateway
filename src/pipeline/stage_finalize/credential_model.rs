//! Credential-model success/failure persistence from context or snapshot.

use super::*;

pub(super) async fn record_credential_model_success(ctx: &PipelineContext, state: &Arc<AppState>) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = ctx.selected_provider_id.clone() else {
        return;
    };
    let Some(model) = ctx
        .resolved_model
        .clone()
        .or_else(|| ctx.canonical_req.requested_model.clone())
    else {
        return;
    };

    let input = db::RecordCredentialModelSuccessInput {
        provider_account_id,
        provider_credential_id: ctx.selected_provider_credential_id.clone(),
        provider_credential_ref: ctx
            .selected_real_credential_ref
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone())
            .or_else(|| ctx.selected_provider_credential_id.clone()),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        model,
    };

    if let Err(error) = db::record_provider_credential_model_success(pg_pool, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to record credential-model success"
        );
    }
}

pub(super) async fn record_credential_model_success_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = snapshot.provider_account_id.clone() else {
        return;
    };
    let Some(model) = snapshot
        .resolved_model
        .clone()
        .or_else(|| snapshot.requested_model.clone())
    else {
        return;
    };

    let input = db::RecordCredentialModelSuccessInput {
        provider_account_id,
        provider_credential_id: snapshot.provider_credential_ref.clone(),
        provider_credential_ref: snapshot
            .real_credential_ref
            .clone()
            .or_else(|| snapshot.provider_credential_ref.clone()),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        model,
    };

    if let Err(error) = db::record_provider_credential_model_success(pg_pool, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to record stream credential-model success"
        );
    }
}

pub(super) async fn record_credential_model_failure(
    ctx: &PipelineContext,
    error: &crate::error::GatewayError,
    upstream_status: Option<u16>,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = ctx.selected_provider_id.clone() else {
        return;
    };
    let Some(model) = ctx
        .resolved_model
        .clone()
        .or_else(|| ctx.canonical_req.requested_model.clone())
    else {
        return;
    };

    let classification = classify_provider_failure(
        upstream_status,
        error.code.as_deref(),
        Some(error.message.as_str()),
    );
    let input = db::RecordCredentialModelFailureInput {
        provider_account_id,
        provider_credential_id: ctx.selected_provider_credential_id.clone(),
        provider_credential_ref: ctx
            .selected_real_credential_ref
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone())
            .or_else(|| ctx.selected_provider_credential_id.clone()),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        model,
        upstream_status,
        error_message: Some(error.message.clone()),
        classification,
    };

    if let Err(error) = db::record_provider_credential_model_failure(pg_pool, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to record credential-model failure"
        );
    }
}

pub(super) async fn record_credential_model_failure_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    error_summary: Option<&str>,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = snapshot.provider_account_id.clone() else {
        return;
    };
    let Some(model) = snapshot
        .resolved_model
        .clone()
        .or_else(|| snapshot.requested_model.clone())
    else {
        return;
    };

    let classification = classify_provider_failure(None, Some("stream_failed"), error_summary);
    let input = db::RecordCredentialModelFailureInput {
        provider_account_id,
        provider_credential_id: snapshot.provider_credential_ref.clone(),
        provider_credential_ref: snapshot
            .real_credential_ref
            .clone()
            .or_else(|| snapshot.provider_credential_ref.clone()),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        model,
        upstream_status: None,
        error_message: error_summary.map(str::to_string),
        classification,
    };

    if let Err(error) = db::record_provider_credential_model_failure(pg_pool, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to record stream credential-model failure"
        );
    }
}
