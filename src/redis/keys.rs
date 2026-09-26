// Single source of truth for all Redis key builders.
// All keys use the "gw:" namespace prefix.

// ---------------------------------------------------------------------------
// Credential cache
// ---------------------------------------------------------------------------

/// Full credential data blob for a given credential ID.
pub fn credential_key(credential_id: &str) -> String {
    format!("gw:cred:{}", credential_id)
}

/// Set of credential IDs belonging to a project.
pub fn credential_project_index_key(project_id: &str) -> String {
    format!("gw:cred:proj:{}", project_id)
}

/// Set of credential IDs belonging to a user.
pub fn credential_user_index_key(user_id: &str) -> String {
    format!("gw:cred:user:{}", user_id)
}

/// Set of credential IDs that can serve a specific model (optional index).
pub fn credential_model_index_key(model: &str) -> String {
    format!("gw:cred:model:{}", model)
}

// ---------------------------------------------------------------------------
// Quota
// ---------------------------------------------------------------------------

/// Remaining quota counter for a credential.
pub fn quota_key(credential_id: &str) -> String {
    format!("gw:quota:{}", credential_id)
}

// ---------------------------------------------------------------------------
// Usage reports
// ---------------------------------------------------------------------------

/// List key for buffered usage report payloads.
pub fn usage_reports_key() -> String {
    "gw:usage:reports".to_string()
}

// ---------------------------------------------------------------------------
// Response cache
// ---------------------------------------------------------------------------

/// Cached response blob keyed by a content hash.
pub fn response_cache_key(hash: &str) -> String {
    format!("gw:rcache:{}", hash)
}

// ---------------------------------------------------------------------------
// API key meta
// ---------------------------------------------------------------------------

/// API key metadata (owner, scopes, expiry, etc.) keyed by key ID.
pub fn api_key_meta_key(key_id: &str) -> String {
    format!("gw:apikey:{}", key_id)
}

// ---------------------------------------------------------------------------
// Provider credentials
// ---------------------------------------------------------------------------

/// Upstream provider payload (credentials / endpoint config) for a provider
/// account.
pub fn provider_payload_key(provider_account_id: &str) -> String {
    format!("gw:provider:payload:{}", provider_account_id)
}

/// Short-lived lock guarding a single provider probe run.
pub fn provider_probe_lock_key(provider_account_id: &str) -> String {
    format!("gw:provider:probe-lock:{}", provider_account_id)
}

/// Rolling failure counter used by the circuit breaker.
pub fn provider_failure_count_key(provider_account_id: &str) -> String {
    format!("gw:provider:failure-count:{}", provider_account_id)
}

/// Circuit breaker open marker for a provider account.
pub fn provider_breaker_open_key(provider_account_id: &str) -> String {
    format!("gw:provider:breaker-open:{}", provider_account_id)
}

/// Cached provider quota snapshot used for routing and operator visibility.
pub fn provider_quota_snapshot_key(provider_account_id: &str) -> String {
    format!("gw:provider:quota:{}", provider_account_id)
}

/// Short-lived lock guarding a single provider quota refresh run.
pub fn provider_quota_lock_key(provider_account_id: &str) -> String {
    format!("gw:provider:quota-lock:{}", provider_account_id)
}

pub fn provider_credential_failure_count_key(provider_credential_id: &str) -> String {
    format!(
        "gw:provider-credential:failure-count:{}",
        provider_credential_id
    )
}

pub fn provider_credential_breaker_open_key(provider_credential_id: &str) -> String {
    format!(
        "gw:provider-credential:breaker-open:{}",
        provider_credential_id
    )
}

pub fn provider_credential_quota_snapshot_key(provider_credential_id: &str) -> String {
    format!("gw:provider-credential:quota:{}", provider_credential_id)
}

pub fn provider_credential_quota_lock_key(provider_credential_id: &str) -> String {
    format!(
        "gw:provider-credential:quota-lock:{}",
        provider_credential_id
    )
}

pub fn provider_credential_folder_sync_status_key() -> String {
    "gw:{provider-credential-folder-sync}:status".to_string()
}

pub fn provider_credential_folder_sync_enabled_key() -> String {
    "gw:{provider-credential-folder-sync}:enabled".to_string()
}

pub fn legacy_provider_credential_folder_sync_status_key() -> &'static str {
    "gw:provider-credential:folder-sync:status"
}

pub fn legacy_provider_credential_folder_sync_enabled_key() -> &'static str {
    "gw:provider-credential:folder-sync:enabled"
}

pub fn provider_credential_folder_sync_events_channel() -> &'static str {
    "gw:provider-credential:folder-sync:events"
}

#[cfg(test)]
mod tests {
    use super::{
        provider_credential_folder_sync_enabled_key, provider_credential_folder_sync_status_key,
    };

    #[test]
    fn folder_sync_state_keys_share_a_redis_cluster_hash_tag() {
        let status = provider_credential_folder_sync_status_key();
        let enabled = provider_credential_folder_sync_enabled_key();
        assert_eq!(
            redis_hash_tag(&status),
            Some("provider-credential-folder-sync")
        );
        assert_eq!(
            redis_hash_tag(&enabled),
            Some("provider-credential-folder-sync")
        );
    }

    fn redis_hash_tag(key: &str) -> Option<&str> {
        let start = key.find('{')?;
        let end = key[start + 1..].find('}')? + start + 1;
        (end > start + 1).then(|| &key[start + 1..end])
    }
}

// ---------------------------------------------------------------------------
// Credential pool refill workflow
// ---------------------------------------------------------------------------

pub fn credential_refill_stream_key() -> &'static str {
    "gw:credential-pool:refill:requests"
}

pub fn credential_refill_pending_tasks_key() -> &'static str {
    "gw:credential-pool:refill:tasks:pending"
}

pub fn credential_refill_recent_tasks_key() -> &'static str {
    "gw:credential-pool:refill:tasks:recent"
}

pub fn credential_refill_task_key(task_id: &str) -> String {
    format!("gw:credential-pool:refill:task:{task_id}")
}

pub fn credential_refill_outstanding_key(provider_id: &str) -> String {
    format!("gw:credential-pool:refill:outstanding:{provider_id}")
}

pub fn credential_refill_lease_key(task_id: &str) -> String {
    format!("gw:credential-pool:refill:lease:{task_id}")
}

pub fn credential_refill_idempotency_key(idempotency_hash: &str) -> String {
    format!("gw:credential-pool:refill:idempotency:{idempotency_hash}")
}

// ---------------------------------------------------------------------------
// Browser executor runtime
// ---------------------------------------------------------------------------

pub fn browser_executor_nodes_key() -> &'static str {
    "gw:browser_executor:nodes"
}

pub fn browser_executor_slots_key() -> &'static str {
    "gw:browser_executor:slots"
}

pub fn browser_executor_leases_key() -> &'static str {
    "gw:browser_executor:leases"
}

pub fn browser_executor_node_key(node_id: &str) -> String {
    format!("gw:browser_executor:node:{}", node_id)
}

pub fn browser_executor_slot_key(slot_id: &str) -> String {
    format!("gw:browser_executor:slot:{}", slot_id)
}

pub fn browser_executor_provider_slots_key(provider_account_id: &str) -> String {
    format!("gw:browser_executor:slots:provider:{}", provider_account_id)
}

pub fn browser_executor_node_slots_key(node_id: &str) -> String {
    format!("gw:browser_executor:slots:node:{}", node_id)
}

pub fn browser_executor_lease_key(lease_id: &str) -> String {
    format!("gw:browser_executor:lease:{}", lease_id)
}

pub fn browser_executor_slot_lease_lock_key(slot_id: &str) -> String {
    format!("gw:browser_executor:slot_lease:{}", slot_id)
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Session data keyed by session ID.
pub fn session_key(session_id: &str) -> String {
    format!("gw:session:{}", session_id)
}

/// Cached user credential projection keyed by the full issued credential key.
pub fn user_credential_key(credential_key: &str) -> String {
    format!("gw:user-cred:{}", credential_key)
}

// ---------------------------------------------------------------------------
// Unified access
// ---------------------------------------------------------------------------

/// Precompiled access projection for a unified access key.
pub fn access_projection_key(access_key_id: &str) -> String {
    format!("gw:access:projection:{}", access_key_id)
}

/// Runtime balance cache for a unified access key.
pub fn access_balance_key(access_key_id: &str) -> String {
    format!("gw:access:balance:{}", access_key_id)
}

/// Sticky affinity for a unified access key and model, optionally session-scoped.
pub fn access_affinity_key(scope: &str, access_key_id: &str, model: &str) -> String {
    format!("gw:access:affinity:{}:{}:{}", scope, access_key_id, model)
}

// ---------------------------------------------------------------------------
// Legacy credential affinity
// ---------------------------------------------------------------------------

/// Legacy provider/credential affinity.
pub fn credential_affinity_key(affinity_scope: &str, model: &str) -> String {
    format!("gw:affinity:{}:{}", affinity_scope, model)
}

// ---------------------------------------------------------------------------
// Credential version (hot-reload)
// ---------------------------------------------------------------------------

/// Global credential version counter. When the platform pushes credential
/// changes, it increments this value. The gateway's background refresh task
/// polls this key and invalidates the in-memory cache when it changes.
pub fn credential_version_key() -> String {
    "gw:cred:version".to_string()
}

// ---------------------------------------------------------------------------
// Gateway console route config transactions
// ---------------------------------------------------------------------------

pub const LEGACY_ROUTE_CONFIG_DOCUMENT_KEY: &str = "gw:config:routes";

pub fn console_route_config_active_revision_key(namespace: &str) -> String {
    format!("gw:console:route-config:{}:active_revision", namespace)
}

pub fn console_route_config_active_document_key(namespace: &str) -> String {
    format!("gw:console:route-config:{}:active_document", namespace)
}

pub fn console_route_config_revision_key(namespace: &str, revision: &str) -> String {
    format!(
        "gw:console:route-config:{}:revisions:{}",
        namespace, revision
    )
}

pub fn console_route_config_transaction_key(namespace: &str, transaction: &str) -> String {
    format!(
        "gw:console:route-config:{}:transactions:{}",
        namespace, transaction
    )
}

pub fn console_route_config_events_key(namespace: &str) -> String {
    format!("gw:console:route-config:{}:events", namespace)
}
