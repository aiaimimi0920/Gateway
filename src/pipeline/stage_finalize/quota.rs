//! Conditional quota settlement and refunds for credentials and access keys.

use super::*;

async fn settle_balance(
    state: &AppState,
    id: &str,
    reserved: u64,
    actual: u64,
) -> Result<(), crate::error::GatewayError> {
    crate::access_balance::AccessBalanceStore::from_state(state)?
        .settle(id, reserved, actual)
        .await
}

async fn refund_balance(
    state: &AppState,
    id: &str,
    reserved: u64,
) -> Result<(), crate::error::GatewayError> {
    crate::access_balance::AccessBalanceStore::from_state(state)?
        .refund(id, reserved)
        .await
}

pub(super) async fn settle_pre_deducted_quota(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    actual_total_tokens: u64,
) {
    let Some(credential_id) = ctx.quota_credential_id.as_deref() else {
        return;
    };
    if ctx.quota_pre_deducted_tokens == 0 {
        return;
    }
    if ctx.requesting_access_key_id.is_some() {
        if let Err(error) = settle_balance(
            state,
            credential_id,
            ctx.quota_pre_deducted_tokens,
            actual_total_tokens,
        )
        .await
        {
            warn!(
                req_id = %ctx.req_id,
                credential_id = %credential_id,
                error = %error,
                "failed to settle unified access key balance"
            );
        }
        return;
    }
    if let Err(error) = settle_quota_after_usage(
        &state.redis_pool,
        credential_id,
        ctx.quota_pre_deducted_tokens,
        actual_total_tokens,
    )
    .await
    {
        warn!(
            req_id = %ctx.req_id,
            credential_id = %credential_id,
            error = %error,
            "failed to settle quota after usage"
        );
    }
}

pub(super) async fn settle_pre_deducted_quota_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    state: &Arc<AppState>,
    actual_total_tokens: u64,
) {
    let Some(credential_id) = snapshot.credential_id.as_deref() else {
        return;
    };
    if snapshot.pre_deducted_tokens == 0 {
        return;
    }
    if snapshot.access_key_id.is_some() {
        if let Err(error) = settle_balance(
            state,
            credential_id,
            snapshot.pre_deducted_tokens,
            actual_total_tokens,
        )
        .await
        {
            warn!(
                request_id = %snapshot.request_id,
                credential_id = %credential_id,
                error = %error,
                "failed to settle unified access key balance for stream success"
            );
        }
        return;
    }
    if let Err(error) = settle_quota_after_usage(
        &state.redis_pool,
        credential_id,
        snapshot.pre_deducted_tokens,
        actual_total_tokens,
    )
    .await
    {
        warn!(
            request_id = %snapshot.request_id,
            credential_id = %credential_id,
            error = %error,
            "failed to settle quota after stream usage"
        );
    }
}

pub(super) async fn refund_pre_deducted_quota(ctx: &PipelineContext, state: &Arc<AppState>) {
    let Some(credential_id) = ctx.quota_credential_id.as_deref() else {
        return;
    };
    if ctx.quota_pre_deducted_tokens == 0 {
        return;
    }
    if ctx.requesting_access_key_id.is_some() {
        if let Err(error) =
            refund_balance(state, credential_id, ctx.quota_pre_deducted_tokens).await
        {
            warn!(
                req_id = %ctx.req_id,
                credential_id = %credential_id,
                error = %error,
                "failed to refund unified access key balance"
            );
        }
        return;
    }
    if let Err(error) = refund_quota(
        &state.redis_pool,
        credential_id,
        ctx.quota_pre_deducted_tokens,
    )
    .await
    {
        warn!(
            req_id = %ctx.req_id,
            credential_id = %credential_id,
            error = %error,
            "failed to refund pre-deducted quota"
        );
    }
}

pub(super) async fn refund_pre_deducted_quota_snapshot(
    snapshot: &FailureFinalizeSnapshot,
    state: &Arc<AppState>,
) {
    let Some(credential_id) = snapshot.credential_id.as_deref() else {
        return;
    };
    if snapshot.pre_deducted_tokens == 0 {
        return;
    }
    if snapshot.request_audit.access_key_id.is_some() {
        if let Err(error) = refund_balance(state, credential_id, snapshot.pre_deducted_tokens).await
        {
            warn!(
                request_id = %snapshot.request_id,
                credential_id = %credential_id,
                error = %error,
                "failed to refund unified access key balance from snapshot"
            );
        }
        return;
    }
    if let Err(error) = refund_quota(
        &state.redis_pool,
        credential_id,
        snapshot.pre_deducted_tokens,
    )
    .await
    {
        warn!(
            request_id = %snapshot.request_id,
            credential_id = %credential_id,
            error = %error,
            "failed to refund stream pre-deducted quota"
        );
    }
}
