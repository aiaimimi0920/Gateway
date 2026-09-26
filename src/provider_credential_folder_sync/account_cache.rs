//! Bounded cache for hydrated provider accounts used during one folder scan.

use crate::db::GatewayProviderAccountView;
use crate::routing::candidate::ProviderExecutionMode;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;
use std::sync::Arc;

const MAX_ENTRIES: usize = 128;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;
const ALLOCATION_OVERHEAD_BYTES: usize = 32;
const OBJECT_NODE_OVERHEAD_BYTES: usize = 1_024;
const OBJECT_ENTRY_OVERHEAD_BYTES: usize = 256;

struct CachedAccount {
    account: Arc<GatewayProviderAccountView>,
    retained_bytes: usize,
}

pub(super) struct HydratedAccountCache {
    entries: HashMap<String, CachedAccount>,
    order: VecDeque<String>,
    retained_bytes: usize,
    max_entries: usize,
    max_retained_bytes: usize,
}

impl HydratedAccountCache {
    pub(super) fn new() -> Self {
        Self::with_limits(MAX_ENTRIES, MAX_RETAINED_BYTES)
    }

    fn with_limits(max_entries: usize, max_retained_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            retained_bytes: 0,
            max_entries,
            max_retained_bytes,
        }
    }

    pub(super) fn get(&mut self, account_id: &str) -> Option<Arc<GatewayProviderAccountView>> {
        let account = self.entries.get(account_id)?.account.clone();
        self.touch(account_id);
        Some(account)
    }

    pub(super) fn insert(
        &mut self,
        account: GatewayProviderAccountView,
    ) -> Arc<GatewayProviderAccountView> {
        let retained_bytes = account_retained_bytes(&account);
        let account_id = account.id.clone();
        let account = Arc::new(account);
        self.remove(&account_id);

        // The caller can use an unusually large account for this file without retaining it.
        if self.max_entries == 0 || retained_bytes > self.max_retained_bytes {
            return account;
        }
        while self.entries.len() >= self.max_entries
            || self
                .retained_bytes
                .checked_add(retained_bytes)
                .is_none_or(|bytes| bytes > self.max_retained_bytes)
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.remove_entry(&oldest);
        }

        self.retained_bytes += retained_bytes;
        self.order.push_back(account_id.clone());
        self.entries.insert(
            account_id,
            CachedAccount {
                account: account.clone(),
                retained_bytes,
            },
        );
        account
    }

    fn touch(&mut self, account_id: &str) {
        self.order.retain(|id| id != account_id);
        self.order.push_back(account_id.to_string());
    }

    fn remove(&mut self, account_id: &str) {
        self.order.retain(|id| id != account_id);
        self.remove_entry(account_id);
    }

    fn remove_entry(&mut self, account_id: &str) {
        if let Some(entry) = self.entries.remove(account_id) {
            self.retained_bytes = self.retained_bytes.saturating_sub(entry.retained_bytes);
        }
    }
}

fn account_retained_bytes(account: &GatewayProviderAccountView) -> usize {
    let mut bytes = size_of::<GatewayProviderAccountView>()
        .saturating_add(size_of::<CachedAccount>().saturating_mul(4))
        .saturating_add(size_of::<String>().saturating_mul(8))
        .saturating_add(ALLOCATION_OVERHEAD_BYTES.saturating_mul(16))
        .saturating_add(account.id.capacity().saturating_mul(3))
        .saturating_add(account.label.capacity())
        .saturating_add(account.service_provider_key.capacity())
        .saturating_add(account.service_provider_label.capacity())
        .saturating_add(account.adapter.capacity())
        .saturating_add(account.protocol_family.capacity())
        .saturating_add(account.protocol_profile.capacity())
        .saturating_add(account.status.capacity())
        .saturating_add(account.storage_mode.capacity());
    for value in [
        &account.source_kind,
        &account.aggregator_api_mode,
        &account.web_reverse_access_mode,
        &account.source_notes,
        &account.cooldown_until,
        &account.last_error,
        &account.last_health_check_at,
    ] {
        bytes = bytes.saturating_add(value.as_ref().map_or(0, |value| {
            value.capacity().saturating_add(ALLOCATION_OVERHEAD_BYTES)
        }));
    }
    bytes = bytes
        .saturating_add(account.created_at.capacity())
        .saturating_add(account.updated_at.capacity())
        .saturating_add(value_heap_bytes(&account.payload));
    if let Some(modes) = &account.endpoint_execution_modes {
        bytes = bytes.saturating_add(
            modes.capacity().saturating_mul(
                size_of::<(String, ProviderExecutionMode)>()
                    .saturating_mul(4)
                    .saturating_add(ALLOCATION_OVERHEAD_BYTES),
            ),
        );
        for key in modes.keys() {
            bytes = bytes.saturating_add(key.capacity());
        }
    }
    bytes
}

fn value_heap_bytes(value: &Value) -> usize {
    match value {
        Value::String(value) => value.capacity().saturating_add(ALLOCATION_OVERHEAD_BYTES),
        Value::Array(values) => values.iter().fold(
            values
                .capacity()
                .saturating_mul(size_of::<Value>())
                .saturating_add(ALLOCATION_OVERHEAD_BYTES),
            |bytes, value| bytes.saturating_add(value_heap_bytes(value)),
        ),
        Value::Object(values) => values.iter().fold(
            values
                .len()
                .saturating_mul(
                    size_of::<String>()
                        .saturating_add(size_of::<Value>())
                        .saturating_add(OBJECT_ENTRY_OVERHEAD_BYTES),
                )
                .saturating_add(OBJECT_NODE_OVERHEAD_BYTES),
            |bytes, (key, value)| {
                bytes
                    .saturating_add(key.capacity())
                    .saturating_add(value_heap_bytes(value))
            },
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn account(id: &str, payload: Value) -> GatewayProviderAccountView {
        GatewayProviderAccountView {
            id: id.to_string(),
            label: id.to_string(),
            service_provider_key: String::new(),
            service_provider_label: String::new(),
            adapter: String::new(),
            protocol_family: String::new(),
            protocol_profile: String::new(),
            status: String::new(),
            source_kind: None,
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload,
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn cache_touches_hits_and_evicts_least_recently_used_entry() {
        let mut cache = HydratedAccountCache::with_limits(2, usize::MAX);
        cache.insert(account("first", json!({"value": 1})));
        cache.insert(account("second", json!({"value": 2})));
        assert!(cache.get("first").is_some());
        cache.insert(account("third", json!({"value": 3})));
        assert!(cache.get("first").is_some());
        assert!(cache.get("second").is_none());
        assert!(cache.get("third").is_some());
    }

    #[test]
    fn cache_evicts_by_retained_byte_budget() {
        let first = account("first", json!({"value": "one"}));
        let second = account("second", json!({"value": "two"}));
        let limit = account_retained_bytes(&first).max(account_retained_bytes(&second));
        let mut cache = HydratedAccountCache::with_limits(2, limit);
        cache.insert(first);
        cache.insert(second);
        assert!(cache.retained_bytes <= limit);
        assert!(cache.get("first").is_none());
        assert!(cache.get("second").is_some());
    }

    #[test]
    fn oversized_account_is_returned_without_being_cached() {
        let account = account("oversized", json!({"value": "not logged"}));
        let limit = account_retained_bytes(&account) - 1;
        let mut cache = HydratedAccountCache::with_limits(2, limit);
        let returned = cache.insert(account);
        assert_eq!(returned.id, "oversized");
        assert!(cache.get("oversized").is_none());
        assert_eq!(cache.retained_bytes, 0);
    }
}
