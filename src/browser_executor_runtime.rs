use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::error::GatewayError;
use crate::redis::keys;
use crate::routing::candidate::ProviderExecutionMode;
use crate::state::AppState;

const DEFAULT_NODE_TTL_SECONDS: u64 = 180;
const DEFAULT_SLOT_TTL_SECONDS: u64 = 600;
const DEFAULT_LEASE_TTL_SECONDS: u64 = 300;
const RELEASED_LEASE_TTL_SECONDS: u64 = 3600;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserExecutorNodeStatus {
    Active,
    Degraded,
    Draining,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserCapabilitySlotStatus {
    Warming,
    Warm,
    Hot,
    Busy,
    Cooling,
    Expired,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorNodeView {
    pub node_id: String,
    pub status: BrowserExecutorNodeStatus,
    pub capabilities: Vec<String>,
    pub last_heartbeat_at: String,
    pub version: Option<String>,
    pub host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilitySlotView {
    pub slot_id: String,
    pub node_id: String,
    pub provider_account_id: String,
    pub adapter: String,
    pub endpoint_kind: String,
    pub execution_mode: ProviderExecutionMode,
    pub status: BrowserCapabilitySlotStatus,
    pub runtime_state_object_key: Option<String>,
    pub account_name: Option<String>,
    pub last_warm_at: Option<String>,
    pub last_used_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub degradation_reasons: Option<Vec<String>>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseView {
    pub lease_id: String,
    pub slot_id: String,
    pub node_id: String,
    pub provider_account_id: String,
    pub request_audit_id: Option<String>,
    pub project_id: Option<String>,
    pub endpoint_kind: String,
    pub execution_mode: ProviderExecutionMode,
    pub issued_at: String,
    pub expires_at: Option<String>,
    pub released_at: Option<String>,
    pub release_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorNodeHealthView {
    pub node_id: String,
    pub total_slots: usize,
    pub warm_slots: usize,
    pub busy_slots: usize,
    pub cooling_slots: usize,
    pub degraded_slots: usize,
    pub challenge_open_slots: usize,
    pub last_error_summary: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorProviderHealthView {
    pub provider_account_id: String,
    pub total_slots: usize,
    pub warm_slots: usize,
    pub busy_slots: usize,
    pub cooling_slots: usize,
    pub degraded_slots: usize,
    pub challenge_open_slots: usize,
    pub last_error_summary: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorHealthView {
    pub generated_at: String,
    pub total_nodes: usize,
    pub total_slots: usize,
    pub total_warm_slots: usize,
    pub total_busy_slots: usize,
    pub total_cooling_slots: usize,
    pub total_degraded_slots: usize,
    pub total_challenge_open_slots: usize,
    pub nodes: Vec<BrowserExecutorNodeHealthView>,
    pub providers: Vec<BrowserExecutorProviderHealthView>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorNodeHeartbeatInput {
    pub node_id: String,
    #[serde(default)]
    pub status: Option<BrowserExecutorNodeStatus>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilitySlotUpsertInput {
    pub slot_id: String,
    pub node_id: String,
    pub provider_account_id: String,
    pub adapter: String,
    pub endpoint_kind: String,
    pub execution_mode: ProviderExecutionMode,
    #[serde(default)]
    pub status: Option<BrowserCapabilitySlotStatus>,
    #[serde(default)]
    pub runtime_state_object_key: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub last_warm_at: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
    #[serde(default)]
    pub last_failure_at: Option<String>,
    #[serde(default)]
    pub degradation_reasons: Option<Vec<String>>,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilitySlotFilters {
    pub node_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub execution_mode: Option<ProviderExecutionMode>,
    pub status: Option<BrowserCapabilitySlotStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseAcquireInput {
    pub provider_account_id: String,
    pub endpoint_kind: String,
    #[serde(default)]
    pub execution_mode: Option<ProviderExecutionMode>,
    #[serde(default)]
    pub request_audit_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub lease_ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseReleaseInput {
    pub lease_id: String,
    #[serde(default)]
    pub release_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseFilters {
    pub provider_account_id: Option<String>,
    pub node_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub request_audit_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorHealthFilters {
    pub provider_account_id: Option<String>,
    pub node_id: Option<String>,
}

pub async fn heartbeat_browser_executor_node(
    state: &AppState,
    input: BrowserExecutorNodeHeartbeatInput,
) -> Result<BrowserExecutorNodeView, GatewayError> {
    let node_id = required_trimmed(&input.node_id, "nodeId")?;
    let view = BrowserExecutorNodeView {
        node_id: node_id.clone(),
        status: input.status.unwrap_or(BrowserExecutorNodeStatus::Active),
        capabilities: normalize_string_array(input.capabilities),
        last_heartbeat_at: now_rfc3339(),
        version: normalize_string(input.version),
        host: normalize_string(input.host),
    };
    let ttl_seconds = input
        .ttl_seconds
        .unwrap_or(DEFAULT_NODE_TTL_SECONDS)
        .max(30);
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let payload = serialize_json(&view, "browser executor node view")?;
    redis::pipe()
        .cmd("SADD")
        .arg(keys::browser_executor_nodes_key())
        .arg(&node_id)
        .ignore()
        .cmd("SET")
        .arg(keys::browser_executor_node_key(&node_id))
        .arg(payload)
        .arg("EX")
        .arg(ttl_seconds)
        .ignore()
        .query_async::<()>(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write browser executor node: {error}"))
        })?;
    Ok(view)
}

pub async fn list_browser_executor_nodes(
    state: &AppState,
) -> Result<Vec<BrowserExecutorNodeView>, GatewayError> {
    let ids = list_members(state, keys::browser_executor_nodes_key()).await?;
    let mut views = read_views_by_ids::<BrowserExecutorNodeView>(
        state,
        keys::browser_executor_nodes_key(),
        &ids,
        keys::browser_executor_node_key,
    )
    .await?;
    views.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    Ok(views)
}

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
    let issued_at = now_rfc3339();
    let expires_at = future_rfc3339(lease_ttl_seconds);

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
            .arg(lease_ttl_seconds + RELEASED_LEASE_TTL_SECONDS)
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

    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let lock_key = keys::browser_executor_slot_lease_lock_key(&lease.slot_id);
    let current_lock: Option<String> = conn.get(&lock_key).await.map_err(|error| {
        GatewayError::server_error(format!("read browser slot lease lock: {error}"))
    })?;
    if current_lock.as_deref() == Some(lease.lease_id.as_str()) {
        let _: usize = conn.del(&lock_key).await.map_err(|error| {
            GatewayError::server_error(format!("release browser slot lease lock: {error}"))
        })?;
    }

    if let Some(slot) = get_browser_capability_slot(state, &lease.slot_id).await? {
        let release_status = choose_release_status(release_reason.as_deref());
        let mut updated_slot = slot;
        updated_slot.status = release_status;
        updated_slot.updated_at = released_at.clone();
        if release_status == BrowserCapabilitySlotStatus::Cooling {
            updated_slot.last_failure_at = Some(released_at.clone());
        }
        let slot_payload = serialize_json(&updated_slot, "browser capability slot view")?;
        let mut conn = state
            .redis_pool
            .get()
            .await
            .map_err(redis_connection_error)?;
        redis::cmd("SET")
            .arg(keys::browser_executor_slot_key(&updated_slot.slot_id))
            .arg(slot_payload)
            .arg("EX")
            .arg(DEFAULT_SLOT_TTL_SECONDS)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("update browser capability slot: {error}"))
            })?;
    }

    let lease_payload = serialize_json(&updated_lease, "browser capability lease view")?;
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    redis::cmd("SET")
        .arg(keys::browser_executor_lease_key(&lease_id))
        .arg(lease_payload)
        .arg("EX")
        .arg(RELEASED_LEASE_TTL_SECONDS)
        .query_async::<()>(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("update browser capability lease: {error}"))
        })?;

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

pub async fn get_browser_executor_health(
    state: &AppState,
    filters: BrowserExecutorHealthFilters,
) -> Result<BrowserExecutorHealthView, GatewayError> {
    let nodes = list_browser_executor_nodes(state).await?;
    let slots = list_browser_capability_slots(
        state,
        BrowserCapabilitySlotFilters {
            provider_account_id: filters.provider_account_id.clone(),
            node_id: filters.node_id.clone(),
            ..BrowserCapabilitySlotFilters::default()
        },
    )
    .await?;

    let filter_node_id = normalize_string(filters.node_id);
    let filtered_nodes = nodes
        .into_iter()
        .filter(|node| {
            filter_node_id
                .as_deref()
                .map(|value| node.node_id == value)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    let mut node_health = std::collections::HashMap::<String, BrowserExecutorNodeHealthView>::new();
    let mut provider_health =
        std::collections::HashMap::<String, BrowserExecutorProviderHealthView>::new();

    for slot in &slots {
        let challenge_open = is_challenge_open(slot);
        let degraded = challenge_open
            || slot
                .degradation_reasons
                .as_ref()
                .map(|reasons| !reasons.is_empty())
                .unwrap_or(false)
            || matches!(
                slot.status,
                BrowserCapabilitySlotStatus::Cooling | BrowserCapabilitySlotStatus::Dead
            );

        let node_entry = node_health.entry(slot.node_id.clone()).or_insert_with(|| {
            BrowserExecutorNodeHealthView {
                node_id: slot.node_id.clone(),
                total_slots: 0,
                warm_slots: 0,
                busy_slots: 0,
                cooling_slots: 0,
                degraded_slots: 0,
                challenge_open_slots: 0,
                last_error_summary: None,
                updated_at: slot.updated_at.clone(),
            }
        });
        update_health_entry(
            node_entry,
            slot,
            challenge_open,
            degraded,
            slot.updated_at.clone(),
        );

        let provider_entry = provider_health
            .entry(slot.provider_account_id.clone())
            .or_insert_with(|| BrowserExecutorProviderHealthView {
                provider_account_id: slot.provider_account_id.clone(),
                total_slots: 0,
                warm_slots: 0,
                busy_slots: 0,
                cooling_slots: 0,
                degraded_slots: 0,
                challenge_open_slots: 0,
                last_error_summary: None,
                updated_at: slot.updated_at.clone(),
            });
        update_health_entry(
            provider_entry,
            slot,
            challenge_open,
            degraded,
            slot.updated_at.clone(),
        );
    }

    let mut node_views = filtered_nodes
        .into_iter()
        .map(|node| {
            node_health
                .remove(&node.node_id)
                .unwrap_or(BrowserExecutorNodeHealthView {
                    node_id: node.node_id,
                    total_slots: 0,
                    warm_slots: 0,
                    busy_slots: 0,
                    cooling_slots: 0,
                    degraded_slots: 0,
                    challenge_open_slots: 0,
                    last_error_summary: None,
                    updated_at: node.last_heartbeat_at,
                })
        })
        .collect::<Vec<_>>();
    node_views.sort_by(|left, right| left.node_id.cmp(&right.node_id));

    let mut provider_views = provider_health.into_values().collect::<Vec<_>>();
    provider_views.sort_by(|left, right| left.provider_account_id.cmp(&right.provider_account_id));

    Ok(BrowserExecutorHealthView {
        generated_at: now_rfc3339(),
        total_nodes: node_views.len(),
        total_slots: slots.len(),
        total_warm_slots: slots
            .iter()
            .filter(|slot| {
                matches!(
                    slot.status,
                    BrowserCapabilitySlotStatus::Warm | BrowserCapabilitySlotStatus::Hot
                )
            })
            .count(),
        total_busy_slots: slots
            .iter()
            .filter(|slot| slot.status == BrowserCapabilitySlotStatus::Busy)
            .count(),
        total_cooling_slots: slots
            .iter()
            .filter(|slot| slot.status == BrowserCapabilitySlotStatus::Cooling)
            .count(),
        total_degraded_slots: slots
            .iter()
            .filter(|slot| {
                slot.degradation_reasons
                    .as_ref()
                    .map(|reasons| !reasons.is_empty())
                    .unwrap_or(false)
                    || matches!(
                        slot.status,
                        BrowserCapabilitySlotStatus::Cooling | BrowserCapabilitySlotStatus::Dead
                    )
                    || is_challenge_open(slot)
            })
            .count(),
        total_challenge_open_slots: slots.iter().filter(|slot| is_challenge_open(slot)).count(),
        nodes: node_views,
        providers: provider_views,
    })
}

trait BrowserExecutorHealthEntry {
    fn total_slots_mut(&mut self) -> &mut usize;
    fn warm_slots_mut(&mut self) -> &mut usize;
    fn busy_slots_mut(&mut self) -> &mut usize;
    fn cooling_slots_mut(&mut self) -> &mut usize;
    fn degraded_slots_mut(&mut self) -> &mut usize;
    fn challenge_open_slots_mut(&mut self) -> &mut usize;
    fn last_error_summary_mut(&mut self) -> &mut Option<String>;
    fn updated_at_mut(&mut self) -> &mut String;
}

impl BrowserExecutorHealthEntry for BrowserExecutorNodeHealthView {
    fn total_slots_mut(&mut self) -> &mut usize {
        &mut self.total_slots
    }
    fn warm_slots_mut(&mut self) -> &mut usize {
        &mut self.warm_slots
    }
    fn busy_slots_mut(&mut self) -> &mut usize {
        &mut self.busy_slots
    }
    fn cooling_slots_mut(&mut self) -> &mut usize {
        &mut self.cooling_slots
    }
    fn degraded_slots_mut(&mut self) -> &mut usize {
        &mut self.degraded_slots
    }
    fn challenge_open_slots_mut(&mut self) -> &mut usize {
        &mut self.challenge_open_slots
    }
    fn last_error_summary_mut(&mut self) -> &mut Option<String> {
        &mut self.last_error_summary
    }
    fn updated_at_mut(&mut self) -> &mut String {
        &mut self.updated_at
    }
}

impl BrowserExecutorHealthEntry for BrowserExecutorProviderHealthView {
    fn total_slots_mut(&mut self) -> &mut usize {
        &mut self.total_slots
    }
    fn warm_slots_mut(&mut self) -> &mut usize {
        &mut self.warm_slots
    }
    fn busy_slots_mut(&mut self) -> &mut usize {
        &mut self.busy_slots
    }
    fn cooling_slots_mut(&mut self) -> &mut usize {
        &mut self.cooling_slots
    }
    fn degraded_slots_mut(&mut self) -> &mut usize {
        &mut self.degraded_slots
    }
    fn challenge_open_slots_mut(&mut self) -> &mut usize {
        &mut self.challenge_open_slots
    }
    fn last_error_summary_mut(&mut self) -> &mut Option<String> {
        &mut self.last_error_summary
    }
    fn updated_at_mut(&mut self) -> &mut String {
        &mut self.updated_at
    }
}

fn update_health_entry<T: BrowserExecutorHealthEntry>(
    entry: &mut T,
    slot: &BrowserCapabilitySlotView,
    challenge_open: bool,
    degraded: bool,
    updated_at: String,
) {
    *entry.total_slots_mut() += 1;
    if matches!(
        slot.status,
        BrowserCapabilitySlotStatus::Warm | BrowserCapabilitySlotStatus::Hot
    ) {
        *entry.warm_slots_mut() += 1;
    }
    if slot.status == BrowserCapabilitySlotStatus::Busy {
        *entry.busy_slots_mut() += 1;
    }
    if slot.status == BrowserCapabilitySlotStatus::Cooling {
        *entry.cooling_slots_mut() += 1;
    }
    if degraded {
        *entry.degraded_slots_mut() += 1;
        let summary = slot
            .degradation_reasons
            .as_ref()
            .map(|reasons| reasons.join(", "))
            .filter(|value| !value.trim().is_empty());
        if summary.is_some() {
            *entry.last_error_summary_mut() = summary;
        }
    }
    if challenge_open {
        *entry.challenge_open_slots_mut() += 1;
    }
    let should_update_timestamp = {
        let current = entry.updated_at_mut().clone();
        updated_at.as_str() > current.as_str()
    };
    if should_update_timestamp {
        *entry.updated_at_mut() = updated_at;
    }
}

async fn get_browser_capability_slot(
    state: &AppState,
    slot_id: &str,
) -> Result<Option<BrowserCapabilitySlotView>, GatewayError> {
    read_view(state, &keys::browser_executor_slot_key(slot_id)).await
}

async fn get_browser_capability_lease(
    state: &AppState,
    lease_id: &str,
) -> Result<Option<BrowserCapabilityLeaseView>, GatewayError> {
    read_view(state, &keys::browser_executor_lease_key(lease_id)).await
}

async fn read_view<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    key: &str,
) -> Result<Option<T>, GatewayError> {
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let raw: Option<String> = conn
        .get(key)
        .await
        .map_err(|error| GatewayError::server_error(format!("read redis key {key}: {error}")))?;
    raw.map(|value| deserialize_json(&value, key)).transpose()
}

async fn list_members(state: &AppState, set_key: &str) -> Result<Vec<String>, GatewayError> {
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let members: Vec<String> = conn.smembers(set_key).await.map_err(|error| {
        GatewayError::server_error(format!("read redis set {set_key}: {error}"))
    })?;
    Ok(members
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect())
}

async fn read_views_by_ids<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    set_key: &str,
    ids: &[String],
    key_builder: impl Fn(&str) -> String,
) -> Result<Vec<T>, GatewayError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let mut stale_ids = Vec::new();
    let mut views = Vec::new();
    for id in ids {
        let key = key_builder(id);
        let raw: Option<String> = conn.get(&key).await.map_err(|error| {
            GatewayError::server_error(format!("read redis key {key}: {error}"))
        })?;
        match raw {
            Some(value) => match deserialize_json(&value, &key) {
                Ok(view) => views.push(view),
                Err(_) => stale_ids.push(id.clone()),
            },
            None => stale_ids.push(id.clone()),
        }
    }
    if !stale_ids.is_empty() {
        let _: usize = conn.srem(set_key, stale_ids).await.map_err(|error| {
            GatewayError::server_error(format!(
                "cleanup stale redis set members {set_key}: {error}"
            ))
        })?;
    }
    Ok(views)
}

fn normalize_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_string_array(values: Option<Vec<String>>) -> Vec<String> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or_default() {
        let trimmed = value.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn normalize_optional_string_array(values: Option<Vec<String>>) -> Option<Vec<String>> {
    let normalized = normalize_string_array(values);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn required_trimmed(value: &str, field_name: &str) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{field_name} 不能为空")));
    }
    Ok(trimmed.to_string())
}

fn serialize_json<T: Serialize>(value: &T, label: &str) -> Result<String, GatewayError> {
    serde_json::to_string(value)
        .map_err(|error| GatewayError::server_error(format!("serialize {label}: {error}")))
}

fn deserialize_json<T: for<'de> Deserialize<'de>>(
    value: &str,
    label: &str,
) -> Result<T, GatewayError> {
    serde_json::from_str(value)
        .map_err(|error| GatewayError::server_error(format!("deserialize {label}: {error}")))
}

fn redis_connection_error(error: deadpool_redis::PoolError) -> GatewayError {
    GatewayError::server_error(format!("get redis connection: {error}"))
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

fn is_slot_lease_eligible(slot: &BrowserCapabilitySlotView) -> bool {
    matches!(
        slot.status,
        BrowserCapabilitySlotStatus::Hot | BrowserCapabilitySlotStatus::Warm
    )
}

fn is_challenge_open(slot: &BrowserCapabilitySlotView) -> bool {
    slot.degradation_reasons
        .as_ref()
        .map(|reasons| {
            reasons
                .iter()
                .any(|reason| reason.to_lowercase().contains("challenge"))
        })
        .unwrap_or(false)
}

fn choose_release_status(release_reason: Option<&str>) -> BrowserCapabilitySlotStatus {
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

fn now_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;

    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "9999-12-31T23:59:59Z".to_string())
}

fn future_rfc3339(offset_seconds: u64) -> String {
    use time::format_description::well_known::Rfc3339;

    (time::OffsetDateTime::now_utc() + time::Duration::seconds(offset_seconds.max(1) as i64))
        .format(&Rfc3339)
        .unwrap_or_else(|_| "9999-12-31T23:59:59Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        choose_release_status, is_challenge_open, BrowserCapabilitySlotStatus,
        BrowserCapabilitySlotView,
    };
    use crate::routing::candidate::ProviderExecutionMode;

    fn sample_slot(
        status: BrowserCapabilitySlotStatus,
        degradation_reasons: Option<Vec<&str>>,
    ) -> BrowserCapabilitySlotView {
        BrowserCapabilitySlotView {
            slot_id: "slot-1".to_string(),
            node_id: "node-1".to_string(),
            provider_account_id: "provider-1".to_string(),
            adapter: "lumalabs_compatible".to_string(),
            endpoint_kind: "images".to_string(),
            execution_mode: ProviderExecutionMode::BrowserBacked,
            status,
            runtime_state_object_key: None,
            account_name: None,
            last_warm_at: None,
            last_used_at: None,
            last_failure_at: None,
            degradation_reasons: degradation_reasons
                .map(|values| values.into_iter().map(|value| value.to_string()).collect()),
            updated_at: "2026-04-14T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn release_status_returns_cooling_for_failure_reasons() {
        assert_eq!(
            choose_release_status(Some("browser challenge timeout")),
            BrowserCapabilitySlotStatus::Cooling
        );
        assert_eq!(
            choose_release_status(Some("hard fail")),
            BrowserCapabilitySlotStatus::Cooling
        );
    }

    #[test]
    fn release_status_returns_warm_for_clean_release() {
        assert_eq!(
            choose_release_status(Some("completed")),
            BrowserCapabilitySlotStatus::Warm
        );
    }

    #[test]
    fn challenge_detection_looks_at_degradation_reasons() {
        assert!(is_challenge_open(&sample_slot(
            BrowserCapabilitySlotStatus::Cooling,
            Some(vec!["challenge_open"])
        )));
        assert!(!is_challenge_open(&sample_slot(
            BrowserCapabilitySlotStatus::Warm,
            Some(vec!["rate_limited"])
        )));
    }
}
