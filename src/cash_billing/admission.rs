//! Reserve the most expensive authorized candidate before any upstream attempt.
use super::{pricing, CashReceipt};
use crate::{
    access_balance::AccessBalanceStore,
    error::GatewayError,
    pipeline::PipelineContext,
    protocol::canonical::{ContentPart, EndpointKind},
    state::AppState,
};
use std::{collections::HashMap, sync::Arc};

pub async fn prepare(ctx: &mut PipelineContext, state: &Arc<AppState>) -> Result<(), GatewayError> {
    let Some(key) = ctx.requesting_access_key_id.as_deref() else {
        return Ok(());
    };
    let store = AccessBalanceStore::from_state(state)?;
    let balance = store.get(key).await?;
    // A unit edit between the first quota check and cash admission must not create an unbilled gap.
    if ctx
        .admitted_access_balance_mode
        .as_deref()
        .is_some_and(|mode| {
            Some(mode)
                != balance
                    .as_ref()
                    .map(|balance| balance.balance_mode.as_str())
        })
    {
        return Err(GatewayError::conflict(
            "Key quota changed during admission; retry the request",
        )
        .with_code("key_quota_changed"));
    }
    if balance
        .as_ref()
        .is_none_or(|balance| balance.balance_mode != "cash_prepaid")
    {
        return Ok(());
    }
    if ctx
        .session
        .as_ref()
        .and_then(|session| session.access_key_kind.as_deref())
        == Some("auto_route")
    {
        return Err(GatewayError::bad_request(
            "Cash billing requires a normal key",
        ));
    }
    let (prompt_bound, completion_bound) = usage_bounds(ctx)?;
    let snapshot = ctx
        .cash_route_snapshot
        .as_ref()
        .ok_or_else(pricing::missing_price)?;
    let metadata = crate::access_store::AccessStore(state)
        .metadata(key)
        .await?;
    let groups = crate::access_key_groups::group_ids(metadata.as_ref())?;
    if ctx.candidates.is_empty() || ctx.candidates.len() > 64 {
        return Err(GatewayError::bad_request(
            "Cash billing requires between 1 and 64 route candidates",
        ));
    }
    let mut payloads = HashMap::new();
    let mut quotes = Vec::new();
    let mut reserved = 0;
    for candidate in &ctx.candidates {
        if !matches!(
            candidate.payload.canonical_adapter(),
            "openai_compatible" | "anthropic_compatible"
        ) || candidate.resolved_execution_mode
            != crate::routing::candidate::ProviderExecutionMode::DirectHttp
        {
            return Err(GatewayError::bad_request(
                "该上游尚未提供现金结算所需的可核对用量与输出上限",
            )
            .with_code("cash_adapter_unsupported"));
        }
        if !payloads.contains_key(&candidate.provider_account_id) {
            payloads.insert(
                candidate.provider_account_id.clone(),
                pricing::payload(state, &candidate.provider_account_id).await?,
            );
        }
        let model = candidate
            .upstream_model
            .as_deref()
            .filter(|model| !model.is_empty())
            .unwrap_or_else(|| {
                crate::upstream::upstream_model_helpers::resolve_model(
                    &candidate.payload,
                    &ctx.canonical_req,
                )
            });
        let quote = pricing::quote(
            snapshot,
            candidate,
            model,
            groups.as_deref(),
            ctx.account_group_id.as_deref(),
            &payloads[&candidate.provider_account_id],
        )?;
        let plan = crate::upstream::client::UpstreamClient::build_request_plan(
            &candidate.payload,
            &ctx.canonical_req,
            model,
            ctx.stream,
        )?;
        let packed = plan.body.as_ref().ok_or_else(bound_error)?;
        let packed_output = output_limit(packed)?;
        let packed_bytes = serde_json::to_vec(packed).map_err(|_| bound_error())?.len() as u64;
        reserved = reserved.max(quote.charge(
            prompt_bound.max(packed_bytes.saturating_add(1024)),
            completion_bound.max(packed_output),
        )?);
        quotes.push(quote);
    }
    let request_id = ctx.req_id.to_string();
    store
        .reserve_cash(CashReceipt {
            request_id: request_id.clone(),
            access_key_id: key.into(),
            created_at: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| super::amount_error())?,
            status: "reserved".into(),
            reserved_micros: reserved,
            amount_micros: None,
            currency: "USD".into(),
            quotes,
            settled_quote: None,
            usage: None,
            reason: None,
        })
        .await?;
    ctx.cash_charge = Some(Arc::new(super::finalization::CashGuard::new(
        request_id,
        state,
        ctx.route_attempt_count.clone(),
    )));
    Ok(())
}

fn usage_bounds(ctx: &PipelineContext) -> Result<(u64, u64), GatewayError> {
    let request = &ctx.canonical_req;
    if !matches!(
        request.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses
    ) || request.previous_response_id.is_some()
        || request
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .any(|part| !matches!(part, ContentPart::Text { .. }))
        || request
            .raw_body
            .get("n")
            .is_some_and(|value| value.as_u64() != Some(1))
        || request
            .raw_body
            .get("best_of")
            .is_some_and(|value| value.as_u64() != Some(1))
    {
        return Err(GatewayError::bad_request(
            "现金限额目前要求单份文本生成、完整输入历史和可确定的用量上限",
        )
        .with_code("cash_usage_bound_unavailable"));
    }
    let output = output_limit(&request.raw_body)?;
    // UTF-8 byte count is intentionally conservative; include canonical framing and tools.
    // If an upstream violates the bound, settlement records all debt and blocks later spending.
    let bytes = serde_json::to_vec(request)
        .map_err(|_| super::amount_error())?
        .len() as u64;
    let framing = (request.messages.len() as u64)
        .checked_mul(64)
        .and_then(|value| value.checked_add(1024))
        .ok_or_else(super::amount_error)?;
    Ok((
        bytes.checked_add(framing).ok_or_else(super::amount_error)?,
        output,
    ))
}

fn bound_error() -> GatewayError {
    GatewayError::bad_request("现金结算要求上游请求携带可核对的输出用量上限")
        .with_code("cash_usage_bound_unavailable")
}

fn output_limit(body: &serde_json::Value) -> Result<u64, GatewayError> {
    let output = ["max_completion_tokens", "max_output_tokens", "max_tokens"]
        .iter()
        .filter_map(|key| body.get(key))
        .map(|value| value.as_u64())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(super::amount_error)?;
    output.into_iter().max().filter(|value| (1..=1_000_000).contains(value))
        .ok_or_else(|| GatewayError::bad_request("使用现金限额时，请设置 max_tokens、max_completion_tokens 或 max_output_tokens（1–1000000）")
            .with_code("cash_output_limit_required"))
}
