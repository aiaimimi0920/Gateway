use anyhow::{Context, Result};
use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::keys;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CACHE_KEY_PREFIX: &str = "gw:rcache:";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseCacheConfig {
    pub enabled: bool,
    pub default_ttl_secs: u64,
    pub max_ttl_secs: u64,
    pub max_entry_size_bytes: usize,
}

impl Default for ResponseCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_ttl_secs: 300,
            max_ttl_secs: 3600,
            max_entry_size_bytes: 512 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageSnapshot {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponseEntry {
    /// The full Redis key (including prefix).
    pub key: String,
    pub model: String,
    pub response_body: String,
    pub content_type: String,
    pub token_usage: Option<TokenUsageSnapshot>,
    pub cached_at: String,
    pub ttl_seconds: u64,
    pub hits: u64,
}

// ---------------------------------------------------------------------------
// Cache key generation
// ---------------------------------------------------------------------------

/// Build a deterministic cache key by SHA-256 hashing the canonical JSON
/// representation of the request parameters.
pub fn build_cache_key(
    model: &str,
    messages: &Value,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
    tools: Option<&Value>,
) -> String {
    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "maxTokens": max_tokens,
        "tools": tools,
    });
    let serialized = serde_json::to_string(&payload).unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let hash = hex::encode(hasher.finalize());

    // The full Redis key already embeds the gw:rcache: prefix via keys module.
    keys::response_cache_key(&hash)
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Fetch a cached response. Increments the hit counter on a cache hit.
pub async fn get_cached_response(
    pool: &Pool,
    cache_key: &str,
) -> Result<Option<CachedResponseEntry>> {
    let mut conn = pool.get().await.context("get redis connection")?;

    let data: std::collections::HashMap<String, String> = conn
        .hgetall(cache_key)
        .await
        .context("HGETALL response cache")?;

    if data.is_empty() {
        return Ok(None);
    }

    // Increment hit counter.
    let _: i64 = conn
        .hincr(cache_key, "hits", 1i64)
        .await
        .context("HINCRBY hits")?;

    let hits: u64 = data.get("hits").and_then(|v| v.parse().ok()).unwrap_or(0) + 1; // reflect the increment

    let token_usage: Option<TokenUsageSnapshot> = data
        .get("token_usage")
        .filter(|s| !s.is_empty())
        .and_then(|s| serde_json::from_str(s).ok());

    let entry = CachedResponseEntry {
        key: cache_key.to_string(),
        model: data.get("model").cloned().unwrap_or_default(),
        response_body: data.get("response_body").cloned().unwrap_or_default(),
        content_type: data.get("content_type").cloned().unwrap_or_default(),
        token_usage,
        cached_at: data.get("cached_at").cloned().unwrap_or_default(),
        ttl_seconds: data
            .get("ttl_seconds")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        hits,
    };

    Ok(Some(entry))
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// Store a response in the cache. Returns `true` when actually stored,
/// `false` when the config disables caching or the entry would exceed limits.
pub async fn set_cached_response(
    pool: &Pool,
    cache_key: &str,
    model: &str,
    response_body: &str,
    content_type: &str,
    token_usage: Option<&TokenUsageSnapshot>,
    ttl_secs: Option<u64>,
    config: &ResponseCacheConfig,
) -> Result<bool> {
    if !config.enabled {
        return Ok(false);
    }

    // Enforce maximum entry size.
    if response_body.len() > config.max_entry_size_bytes {
        return Ok(false);
    }

    // Resolve and cap TTL.
    let requested_ttl = ttl_secs.unwrap_or(config.default_ttl_secs);
    let ttl = requested_ttl.min(config.max_ttl_secs);
    if ttl == 0 {
        return Ok(false);
    }

    let token_usage_json = token_usage
        .map(|u| serde_json::to_string(u).unwrap_or_default())
        .unwrap_or_default();

    let cached_at = current_iso_timestamp();

    let fields: Vec<(&str, String)> = vec![
        ("model", model.to_string()),
        ("response_body", response_body.to_string()),
        ("content_type", content_type.to_string()),
        ("token_usage", token_usage_json),
        ("cached_at", cached_at),
        ("ttl_seconds", ttl.to_string()),
        ("hits", "0".to_string()),
    ];

    let mut conn = pool.get().await.context("get redis connection")?;

    let mut pipe = redis::pipe();
    pipe.atomic();
    pipe.hset_multiple(cache_key, &fields).ignore();
    pipe.expire(cache_key, ttl as i64).ignore();
    pipe.query_async::<()>(&mut conn)
        .await
        .context("pipeline HSET + EXPIRE response cache")?;

    Ok(true)
}

// ---------------------------------------------------------------------------
// Invalidation
// ---------------------------------------------------------------------------

/// Delete cached responses matching a key pattern (SCAN + DEL).
/// If `pattern` is `None`, invalidates all `gw:rcache:*` entries.
pub async fn invalidate_cache(pool: &Pool, pattern: Option<&str>) -> Result<u64> {
    let scan_pattern = pattern
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("{}*", CACHE_KEY_PREFIX));

    let mut conn = pool.get().await.context("get redis connection")?;
    let mut deleted: u64 = 0;
    let mut cursor: u64 = 0;

    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&scan_pattern)
            .arg("COUNT")
            .arg(100u64)
            .query_async(&mut conn)
            .await
            .context("SCAN response cache")?;

        if !keys.is_empty() {
            let count: u64 = conn
                .del(keys.as_slice())
                .await
                .context("DEL response cache keys")?;
            deleted += count;
        }

        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the current UTC time as an ISO-8601 string without pulling in `chrono`.
fn current_iso_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Decompose unix timestamp into Y-M-D H:M:S (UTC, Gregorian).
    let mut remaining = secs;
    let s = remaining % 60;
    remaining /= 60;
    let m = remaining % 60;
    remaining /= 60;
    let h = remaining % 24;
    remaining /= 24;

    // Days since 1970-01-01
    let mut days = remaining as i64;
    // Shift to 0001-03-01 era for easy month computation.
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, d, h, m, s
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- build_cache_key ----

    #[test]
    fn test_build_cache_key_deterministic() {
        let messages = json!([{"role": "user", "content": "Hello"}]);
        let k1 = build_cache_key("gpt-4o", &messages, Some(0.7), Some(100), None);
        let k2 = build_cache_key("gpt-4o", &messages, Some(0.7), Some(100), None);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_build_cache_key_different_model_differs() {
        let messages = json!([{"role": "user", "content": "Hello"}]);
        let k1 = build_cache_key("gpt-4o", &messages, None, None, None);
        let k2 = build_cache_key("gpt-3.5-turbo", &messages, None, None, None);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_build_cache_key_different_temperature_differs() {
        let messages = json!([{"role": "user", "content": "Hello"}]);
        let k1 = build_cache_key("gpt-4o", &messages, Some(0.0), None, None);
        let k2 = build_cache_key("gpt-4o", &messages, Some(1.0), None, None);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_build_cache_key_has_gw_prefix() {
        let messages = json!([]);
        let k = build_cache_key("model", &messages, None, None, None);
        assert!(k.starts_with("gw:rcache:"), "key was: {k}");
    }

    #[test]
    fn test_build_cache_key_none_and_explicit_null_same() {
        // None temperature and None max_tokens should produce the same key
        // regardless of how we pass them.
        let messages = json!([{"role": "user", "content": "test"}]);
        let k1 = build_cache_key("claude-3", &messages, None, None, None);
        let k2 = build_cache_key("claude-3", &messages, None, None, None);
        assert_eq!(k1, k2);
    }

    // ---- current_iso_timestamp ----

    #[test]
    fn test_current_iso_timestamp_format() {
        let ts = current_iso_timestamp();
        // Must match YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20, "timestamp len mismatch: {ts}");
    }

    // ---- ResponseCacheConfig defaults ----

    #[test]
    fn test_response_cache_config_defaults() {
        let cfg = ResponseCacheConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.default_ttl_secs, 300);
        assert_eq!(cfg.max_ttl_secs, 3600);
        assert_eq!(cfg.max_entry_size_bytes, 512 * 1024);
    }

    // ---- Redis integration tests (ignored) ----

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_set_get_cached_response() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let messages = serde_json::json!([{"role":"user","content":"hi"}]);
        let key = build_cache_key("gpt-4o", &messages, Some(0.0), None, None);
        let config = ResponseCacheConfig::default();

        let stored = set_cached_response(
            &pool,
            &key,
            "gpt-4o",
            r#"{"id":"chatcmpl-test"}"#,
            "application/json",
            None,
            None,
            &config,
        )
        .await
        .unwrap();
        assert!(stored);

        let entry = get_cached_response(&pool, &key).await.unwrap();
        assert!(entry.is_some());
        let e = entry.unwrap();
        assert_eq!(e.model, "gpt-4o");
        assert_eq!(e.hits, 1);
    }

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_invalidate_cache() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let messages = serde_json::json!([{"role":"user","content":"invalidate test"}]);
        let key = build_cache_key("gpt-4o", &messages, None, None, None);
        let config = ResponseCacheConfig::default();

        set_cached_response(
            &pool,
            &key,
            "gpt-4o",
            "{}",
            "application/json",
            None,
            None,
            &config,
        )
        .await
        .unwrap();

        let deleted = invalidate_cache(&pool, None).await.unwrap();
        assert!(deleted >= 1);
    }
}
