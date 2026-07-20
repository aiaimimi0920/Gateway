// ---------------------------------------------------------------------------
// credential_store.rs — In-memory credential cache (Tier 1)
//
// Sits in front of Redis for hot-path credential lookups. Uses DashMap for
// lock-free concurrent reads and a configurable TTL for cache expiry.
// ---------------------------------------------------------------------------

use dashmap::DashMap;
use std::time::{Duration, Instant};

use crate::redis::credential_cache::CredentialEntry;

/// In-memory credential cache with TTL.
/// Sits in front of Redis for hot-path lookups.
///
/// `Clone` is cheap: `DashMap` is internally `Arc`-wrapped, so clones share
/// the same underlying map. This allows passing a clone to background tasks
/// (e.g. the credential refresh watcher) without wrapping in an extra `Arc`.
#[derive(Clone)]
pub struct CredentialMemoryCache {
    /// Key: "{project_id}:{model}" -> (entries, cached_at)
    cache: DashMap<String, (Vec<CredentialEntry>, Instant)>,
    ttl: Duration,
}

impl CredentialMemoryCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            cache: DashMap::new(),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Get cached credentials. Returns None on miss or expiry.
    pub fn get(&self, project_id: &str, model: &str) -> Option<Vec<CredentialEntry>> {
        let key = format!("{}:{}", project_id, model);
        let entry = self.cache.get(&key)?;
        let (creds, cached_at) = entry.value();
        if cached_at.elapsed() > self.ttl {
            drop(entry);
            self.cache.remove(&key);
            return None;
        }
        Some(creds.clone())
    }

    /// Store credentials in cache.
    pub fn put(&self, project_id: &str, model: &str, entries: Vec<CredentialEntry>) {
        let key = format!("{}:{}", project_id, model);
        self.cache.insert(key, (entries, Instant::now()));
    }

    /// Invalidate all entries for a project.
    pub fn invalidate_project(&self, project_id: &str) {
        let prefix = format!("{}:", project_id);
        self.cache.retain(|k, _| !k.starts_with(&prefix));
    }

    /// Invalidate everything.
    pub fn invalidate_all(&self) {
        self.cache.clear();
    }

    /// Number of entries currently in the cache (for /metrics).
    pub fn entry_count(&self) -> usize {
        self.cache.len()
    }
}

// ---------------------------------------------------------------------------
// Background credential refresh task
// ---------------------------------------------------------------------------

/// Start a background task that periodically checks Redis for credential
/// changes and invalidates the in-memory cache.
///
/// Uses a simple polling approach (every `interval_secs` seconds) rather than
/// Pub/Sub to avoid Redis Pub/Sub complexity and connection management.
///
/// The task checks a Redis key `gw:cred:version` -- when the platform pushes
/// credential changes, it increments this counter. If the counter changed
/// since last check, the in-memory cache is fully invalidated.
pub async fn start_credential_refresh_task(
    cache: CredentialMemoryCache,
    pool: deadpool_redis::Pool,
    interval_secs: u64,
) {
    let mut last_version: i64 = 0;
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

    loop {
        interval.tick().await;

        // Check Redis version counter.
        let current_version: i64 = match pool.get().await {
            Ok(mut conn) => redis::cmd("GET")
                .arg(crate::redis::keys::credential_version_key())
                .query_async(&mut conn)
                .await
                .unwrap_or(0),
            Err(e) => {
                tracing::debug!("credential refresh: Redis connection error: {}", e);
                continue;
            }
        };

        if current_version != last_version && last_version != 0 {
            tracing::info!(
                old_version = last_version,
                new_version = current_version,
                "credential version changed -- invalidating cache"
            );
            cache.invalidate_all();
        }

        last_version = current_version;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redis::credential_cache::CredentialKind;

    fn make_entry(id: &str) -> CredentialEntry {
        CredentialEntry {
            id: id.to_string(),
            kind: CredentialKind::PlatformUnlimited,
            project_id: "proj-1".to_string(),
            user_id: "user-1".to_string(),
            provider: "openai".to_string(),
            api_key: Some("sk-test".to_string()),
            api_base_url: Some("https://api.openai.com".to_string()),
            headers: None,
            account_payload: None,
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = CredentialMemoryCache::new(30);
        assert!(cache.get("proj-1", "gpt-4o").is_none());
    }

    #[test]
    fn cache_hit_returns_entries() {
        let cache = CredentialMemoryCache::new(30);
        let entries = vec![make_entry("cred-1"), make_entry("cred-2")];
        cache.put("proj-1", "gpt-4o", entries.clone());

        let result = cache.get("proj-1", "gpt-4o");
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn cache_different_model_is_separate() {
        let cache = CredentialMemoryCache::new(30);
        cache.put("proj-1", "gpt-4o", vec![make_entry("cred-1")]);

        assert!(cache.get("proj-1", "gpt-4o").is_some());
        assert!(cache.get("proj-1", "claude-sonnet").is_none());
    }

    #[test]
    fn invalidate_project_clears_all_models() {
        let cache = CredentialMemoryCache::new(30);
        cache.put("proj-1", "gpt-4o", vec![make_entry("c1")]);
        cache.put("proj-1", "claude-sonnet", vec![make_entry("c2")]);
        cache.put("proj-2", "gpt-4o", vec![make_entry("c3")]);

        cache.invalidate_project("proj-1");

        assert!(cache.get("proj-1", "gpt-4o").is_none());
        assert!(cache.get("proj-1", "claude-sonnet").is_none());
        // proj-2 should be untouched
        assert!(cache.get("proj-2", "gpt-4o").is_some());
    }

    #[test]
    fn invalidate_all_clears_everything() {
        let cache = CredentialMemoryCache::new(30);
        cache.put("proj-1", "gpt-4o", vec![make_entry("c1")]);
        cache.put("proj-2", "claude", vec![make_entry("c2")]);

        cache.invalidate_all();

        assert!(cache.get("proj-1", "gpt-4o").is_none());
        assert!(cache.get("proj-2", "claude").is_none());
    }

    #[test]
    fn expired_entries_are_evicted() {
        let cache = CredentialMemoryCache::new(0); // 0-second TTL = immediate expiry
        cache.put("proj-1", "gpt-4o", vec![make_entry("c1")]);

        // With 0s TTL, the entry should be expired by the time we read it
        // (elapsed > 0)
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(cache.get("proj-1", "gpt-4o").is_none());
    }
}
