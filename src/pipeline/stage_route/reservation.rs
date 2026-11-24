//! Process-owned auto-route reservation. Durable SAP selects without this owner;
//! the ordinary branch reserves the already-selected source exactly once.
use super::{AppState, GatewayError, PipelineContext};
use std::sync::Arc;

pub(crate) async fn reserve_preselected_auto(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<(), GatewayError> {
    let selected = ctx.projected_access_candidates.first().cloned();
    reserve(ctx, state, selected.as_ref()).await
}

pub(super) async fn reserve(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    selected: Option<&crate::db::ProjectedPlatformAccessRow>,
) -> Result<(), GatewayError> {
    if ctx
        .session
        .as_ref()
        .and_then(|s| s.access_key_kind.as_deref())
        != Some("auto_route")
    {
        return Ok(());
    }
    if let (Some(selected), Some(pg_pool)) = (selected, state.pg_pool.as_ref()) {
        let estimated =
            crate::redis::usage_tracking::estimate_token_count(&ctx.canonical_req.messages_text())
                .max(1);
        let decision = crate::db::pre_deduct_access_key_balance(
            pg_pool,
            &state.redis_pool,
            &selected.source_access_key_id,
            estimated,
        )
        .await?;
        if !decision.allowed {
            return Err(
                GatewayError::quota_exceeded("当前自动路由 key 的候选额度不足").with_code(
                    decision
                        .reason
                        .unwrap_or_else(|| "balance_not_allowed".to_string()),
                ),
            );
        }
        ctx.quota_credential_id = Some(selected.source_access_key_id.clone());
        ctx.quota_pre_deducted_tokens = decision.pre_deduct_amount;
    }
    Ok(())
}
