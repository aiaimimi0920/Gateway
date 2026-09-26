use super::normalization::{normalize_optional_string_array, normalize_string, required_trimmed};
use super::storage::{
    get_browser_capability_slot, list_members, read_views_by_ids, redis_connection_error,
    serialize_json,
};
use super::timestamps::now_rfc3339;
use super::{
    BrowserCapabilitySlotFilters, BrowserCapabilitySlotStatus, BrowserCapabilitySlotUpsertInput,
    BrowserCapabilitySlotView, DEFAULT_SLOT_TTL_SECONDS,
};
use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
pub async fn upsert_browser_capability_slot(
    state: &AppState,
    input: BrowserCapabilitySlotUpsertInput,
) -> Result<BrowserCapabilitySlotView, GatewayError> {
    let slot_id = required_trimmed(&input.slot_id, "slotId")?;
    let node_id = required_trimmed(&input.node_id, "nodeId")?;
    let provider_account_id = required_trimmed(&input.provider_account_id, "providerAccountId")?;
    let adapter = required_trimmed(&input.adapter, "adapter")?;
    let endpoint_kind = required_trimmed(&input.endpoint_kind, "endpointKind")?;

    let existing = get_browser_capability_slot(state, &slot_id).await?;
    let view = BrowserCapabilitySlotView {
        slot_id: slot_id.clone(),
        node_id: node_id.clone(),
        provider_account_id: provider_account_id.clone(),
        adapter,
        endpoint_kind,
        execution_mode: input.execution_mode,
        status: input.status.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|view| view.status)
                .unwrap_or(BrowserCapabilitySlotStatus::Warming)
        }),
        runtime_state_object_key: normalize_string(input.runtime_state_object_key).or_else(|| {
            existing
                .as_ref()
                .and_then(|view| view.runtime_state_object_key.clone())
        }),
        account_name: normalize_string(input.account_name)
            .or_else(|| existing.as_ref().and_then(|view| view.account_name.clone())),
        last_warm_at: normalize_string(input.last_warm_at)
            .or_else(|| existing.as_ref().and_then(|view| view.last_warm_at.clone())),
        last_used_at: normalize_string(input.last_used_at)
            .or_else(|| existing.as_ref().and_then(|view| view.last_used_at.clone())),
        last_failure_at: normalize_string(input.last_failure_at).or_else(|| {
            existing
                .as_ref()
                .and_then(|view| view.last_failure_at.clone())
        }),
        degradation_reasons: normalize_optional_string_array(input.degradation_reasons.or_else(
            || {
                existing
                    .as_ref()
                    .and_then(|view| view.degradation_reasons.clone())
            },
        )),
        updated_at: now_rfc3339(),
    };

    let ttl_seconds = input
        .ttl_seconds
        .unwrap_or(DEFAULT_SLOT_TTL_SECONDS)
        .max(60);
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let payload = serialize_json(&view, "browser capability slot view")?;
    let mut pipe = redis::pipe();
    pipe.cmd("SADD")
        .arg(keys::browser_executor_slots_key())
        .arg(&slot_id)
        .ignore()
        .cmd("SADD")
        .arg(keys::browser_executor_node_slots_key(&node_id))
        .arg(&slot_id)
        .ignore()
        .cmd("SADD")
        .arg(keys::browser_executor_provider_slots_key(
            &provider_account_id,
        ))
        .arg(&slot_id)
        .ignore()
        .cmd("SET")
        .arg(keys::browser_executor_slot_key(&slot_id))
        .arg(payload)
        .arg("EX")
        .arg(ttl_seconds)
        .ignore();

    if let Some(existing) = existing.as_ref() {
        if existing.node_id != node_id {
            pipe.cmd("SREM")
                .arg(keys::browser_executor_node_slots_key(&existing.node_id))
                .arg(&slot_id)
                .ignore();
        }
        if existing.provider_account_id != provider_account_id {
            pipe.cmd("SREM")
                .arg(keys::browser_executor_provider_slots_key(
                    &existing.provider_account_id,
                ))
                .arg(&slot_id)
                .ignore();
        }
    }

    pipe.query_async::<()>(&mut conn).await.map_err(|error| {
        GatewayError::server_error(format!("write browser capability slot: {error}"))
    })?;
    Ok(view)
}

pub async fn list_browser_capability_slots(
    state: &AppState,
    filters: BrowserCapabilitySlotFilters,
) -> Result<Vec<BrowserCapabilitySlotView>, GatewayError> {
    let provider_account_id = normalize_string(filters.provider_account_id);
    let ids = if let Some(provider_account_id) = provider_account_id.as_deref() {
        list_members(
            state,
            &keys::browser_executor_provider_slots_key(provider_account_id),
        )
        .await?
    } else {
        list_members(state, keys::browser_executor_slots_key()).await?
    };
    let mut views = read_views_by_ids::<BrowserCapabilitySlotView>(
        state,
        keys::browser_executor_slots_key(),
        &ids,
        keys::browser_executor_slot_key,
    )
    .await?;
    let node_id = normalize_string(filters.node_id);
    let endpoint_kind = normalize_string(filters.endpoint_kind);
    views.retain(|view| {
        if node_id
            .as_deref()
            .is_some_and(|value| view.node_id != value)
        {
            return false;
        }
        if provider_account_id
            .as_deref()
            .is_some_and(|value| view.provider_account_id != value)
        {
            return false;
        }
        if endpoint_kind
            .as_deref()
            .is_some_and(|value| view.endpoint_kind != value)
        {
            return false;
        }
        if filters
            .execution_mode
            .is_some_and(|value| view.execution_mode != value)
        {
            return false;
        }
        if filters.status.is_some_and(|value| view.status != value) {
            return false;
        }
        true
    });
    views.sort_by(|left, right| {
        let left_priority = slot_status_priority(left.status);
        let right_priority = slot_status_priority(right.status);
        left_priority
            .cmp(&right_priority)
            .then_with(|| left.slot_id.cmp(&right.slot_id))
    });
    Ok(views)
}

fn slot_status_priority(status: BrowserCapabilitySlotStatus) -> usize {
    match status {
        BrowserCapabilitySlotStatus::Hot => 0,
        BrowserCapabilitySlotStatus::Warm => 1,
        BrowserCapabilitySlotStatus::Warming => 2,
        BrowserCapabilitySlotStatus::Busy => 3,
        BrowserCapabilitySlotStatus::Cooling => 4,
        BrowserCapabilitySlotStatus::Expired => 5,
        BrowserCapabilitySlotStatus::Dead => 6,
    }
}
