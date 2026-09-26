use super::lease_release::commit_lease_release;
use super::normalization::{normalize_string, required_trimmed};
use super::slots::list_browser_capability_slots;
use super::storage::{
    get_browser_capability_lease, list_members, read_views_by_ids, redis_connection_error,
    serialize_json,
};
use super::timestamps::{future_rfc3339, now_rfc3339};
use super::{
    BrowserCapabilityLeaseAcquireInput, BrowserCapabilityLeaseFilters,
    BrowserCapabilityLeaseReleaseInput, BrowserCapabilityLeaseView, BrowserCapabilitySlotFilters,
    BrowserCapabilitySlotStatus, BrowserCapabilitySlotView, DEFAULT_LEASE_TTL_SECONDS,
    DEFAULT_SLOT_TTL_SECONDS, RELEASED_LEASE_TTL_SECONDS,
};
use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
pub async fn acquire_browser_capability_lease(
    state: &AppState,
    input: BrowserCapabilityLeaseAcquireInput,
) -> Result<BrowserCapabilityLeaseView, GatewayError> {
    let provider_account_id = required_trimmed(&input.provider_account_id, "providerAccountId")?;
    let endpoint_kind = required_trimmed(&input.endpoint_kind, "endpointKind")?;
    let slots = list_browser_capability_slots(
        state,
        BrowserCapabilitySlotFilters {
            provider_account_id: Some(provider_account_id.clone()),
            endpoint_kind: Some(endpoint_kind.clone()),
            execution_mode: input.execution_mode,
            ..BrowserCapabilitySlotFilters::default()
        },
    )
    .await?;
    let candidates = slots
        .into_iter()
        .filter(is_slot_lease_eligible)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(GatewayError::conflict(
            "当前没有可租赁的 browser capability slot。",
        ));
    }

    let lease_ttl_seconds = input
        .lease_ttl_seconds
        .unwrap_or(DEFAULT_LEASE_TTL_SECONDS)
        .max(30);
    let retained_ttl_seconds = lease_ttl_seconds
        .checked_add(RELEASED_LEASE_TTL_SECONDS)
        .ok_or_else(|| GatewayError::bad_request("leaseTtlSeconds exceeds the retention range"))?;
    let issued_at = now_rfc3339();
    let expires_at = future_rfc3339(lease_ttl_seconds)?;

    for slot in candidates {
        let lease_id = uuid::Uuid::new_v4().to_string();
        let lock_key = keys::browser_executor_slot_lease_lock_key(&slot.slot_id);
        let mut conn = state
            .redis_pool
            .get()
            .await
            .map_err(redis_connection_error)?;
        let acquired: Option<String> = redis::cmd("SET")
            .arg(&lock_key)
            .arg(&lease_id)
            .arg("EX")
            .arg(lease_ttl_seconds)
            .arg("NX")
            .query_async(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("acquire browser slot lease lock: {error}"))
            })?;
        if acquired.as_deref() != Some("OK") {
            continue;
        }

        let lease = BrowserCapabilityLeaseView {
            lease_id: lease_id.clone(),
            slot_id: slot.slot_id.clone(),
            node_id: slot.node_id.clone(),
            provider_account_id: slot.provider_account_id.clone(),
            request_audit_id: normalize_string(input.request_audit_id.clone()),
            project_id: normalize_string(input.project_id.clone()),
            endpoint_kind: slot.endpoint_kind.clone(),
            execution_mode: slot.execution_mode,
            issued_at: issued_at.clone(),
            expires_at: Some(expires_at.clone()),
            released_at: None,
            release_reason: None,
        };
        let updated_slot = BrowserCapabilitySlotView {
            status: BrowserCapabilitySlotStatus::Busy,
            last_used_at: Some(issued_at.clone()),
            updated_at: issued_at.clone(),
            ..slot
        };
        let lease_payload = serialize_json(&lease, "browser capability lease view")?;
        let slot_payload = serialize_json(&updated_slot, "browser capability slot view")?;
        redis::pipe()
            .cmd("SADD")
            .arg(keys::browser_executor_leases_key())
            .arg(&lease_id)
            .ignore()
            .cmd("SET")
            .arg(keys::browser_executor_lease_key(&lease_id))
            .arg(lease_payload)
            .arg("EX")
            .arg(retained_ttl_seconds)
            .ignore()
            .cmd("SET")
            .arg(keys::browser_executor_slot_key(&lease.slot_id))
            .arg(slot_payload)
            .arg("EX")
            .arg(DEFAULT_SLOT_TTL_SECONDS)
            .ignore()
            .query_async::<()>(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("persist browser capability lease: {error}"))
            })?;
        return Ok(lease);
    }

    Err(GatewayError::conflict(
        "当前没有可用的 browser capability slot lease。",
    ))
}

pub async fn release_browser_capability_lease(
    state: &AppState,
    input: BrowserCapabilityLeaseReleaseInput,
) -> Result<BrowserCapabilityLeaseView, GatewayError> {
    let lease_id = required_trimmed(&input.lease_id, "leaseId")?;
    let Some(lease) = get_browser_capability_lease(state, &lease_id).await? else {
        return Err(GatewayError::not_found("Browser capability lease 不存在。"));
    };

    let released_at = now_rfc3339();
    let release_reason = normalize_string(input.release_reason);
    let updated_lease = BrowserCapabilityLeaseView {
        released_at: Some(released_at.clone()),
        release_reason: release_reason.clone(),
        ..lease.clone()
    };

    commit_lease_release(
        state,
        &updated_lease,
        &released_at,
        choose_release_status(release_reason.as_deref()),
    )
    .await?;

    Ok(updated_lease)
}

pub async fn list_browser_capability_leases(
    state: &AppState,
    filters: BrowserCapabilityLeaseFilters,
) -> Result<Vec<BrowserCapabilityLeaseView>, GatewayError> {
    let ids = list_members(state, keys::browser_executor_leases_key()).await?;
    let mut views = read_views_by_ids::<BrowserCapabilityLeaseView>(
        state,
        keys::browser_executor_leases_key(),
        &ids,
        keys::browser_executor_lease_key,
    )
    .await?;
    let provider_account_id = normalize_string(filters.provider_account_id);
    let node_id = normalize_string(filters.node_id);
    let endpoint_kind = normalize_string(filters.endpoint_kind);
    let request_audit_id = normalize_string(filters.request_audit_id);
    views.retain(|view| {
        if provider_account_id
            .as_deref()
            .is_some_and(|value| view.provider_account_id != value)
        {
            return false;
        }
        if node_id
            .as_deref()
            .is_some_and(|value| view.node_id != value)
        {
            return false;
        }
        if endpoint_kind
            .as_deref()
            .is_some_and(|value| view.endpoint_kind != value)
        {
            return false;
        }
        if request_audit_id
            .as_deref()
            .is_some_and(|value| view.request_audit_id.as_deref() != Some(value))
        {
            return false;
        }
        true
    });
    views.sort_by(|left, right| right.issued_at.cmp(&left.issued_at));
    Ok(views)
}

fn is_slot_lease_eligible(slot: &BrowserCapabilitySlotView) -> bool {
    matches!(
        slot.status,
        BrowserCapabilitySlotStatus::Hot | BrowserCapabilitySlotStatus::Warm
    )
}

pub(super) fn choose_release_status(release_reason: Option<&str>) -> BrowserCapabilitySlotStatus {
    let reason = release_reason
        .map(str::trim)
        .unwrap_or_default()
        .to_lowercase();
    if reason.contains("crash")
        || reason.contains("timeout")
        || reason.contains("challenge")
        || reason.contains("error")
        || reason.contains("fail")
    {
        BrowserCapabilitySlotStatus::Cooling
    } else {
        BrowserCapabilitySlotStatus::Warm
    }
}
