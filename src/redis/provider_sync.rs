use anyhow::{Context, Result};
use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde_json::Value;

use super::keys;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default cache TTL for provider payloads: 10 minutes.
/// Provider credentials change rarely but we don't want stale data forever.
const DEFAULT_TTL_SECS: u64 = 600;

/// Key prefix used for SCAN-based bulk invalidation.
const PROVIDER_PAYLOAD_PREFIX: &str = "gw:provider:payload:";

// ---------------------------------------------------------------------------
// Cache Read
// ---------------------------------------------------------------------------

/// Read a provider account payload from the Redis cache.
/// Returns `None` on cache miss or JSON parse failure.
pub async fn get_cached_provider_payload(pool: &Pool, provider_id: &str) -> Result<Option<Value>> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let raw: Option<String> = conn
        .get(keys::provider_payload_key(provider_id))
        .await
        .context("GET provider payload")?;

    match raw {
        None => Ok(None),
        Some(s) => {
            let v: Value = serde_json::from_str(&s).context("deserialize provider payload")?;
            Ok(Some(v))
        }
    }
}

// ---------------------------------------------------------------------------
// Cache Write
// ---------------------------------------------------------------------------

/// Cache a provider account payload in Redis with a TTL.
/// Uses `DEFAULT_TTL_SECS` (600 s) when `ttl_secs` is `None`.
pub async fn set_cached_provider_payload(
    pool: &Pool,
    provider_id: &str,
    payload: &Value,
    ttl_secs: Option<u64>,
) -> Result<()> {
    let ttl = ttl_secs.unwrap_or(DEFAULT_TTL_SECS);
    let key = keys::provider_payload_key(provider_id);
    let serialized = serde_json::to_string(payload).context("serialize provider payload")?;

    let mut conn = pool.get().await.context("get redis connection")?;
    redis::cmd("SET")
        .arg(&key)
        .arg(&serialized)
        .arg("EX")
        .arg(ttl)
        .query_async::<()>(&mut conn)
        .await
        .context("SET provider payload with EX")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Cache Delete
// ---------------------------------------------------------------------------

/// Remove a provider account payload from the Redis cache.
pub async fn delete_cached_provider_payload(pool: &Pool, provider_id: &str) -> Result<()> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let _: u64 = conn
        .del(keys::provider_payload_key(provider_id))
        .await
        .context("DEL provider payload")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Batch Sync
// ---------------------------------------------------------------------------

/// Sync multiple provider payloads to Redis in a single pipeline.
///
/// Each entry is `(provider_id, payload)`. Returns the number of entries
/// successfully written.
pub async fn sync_provider_payloads_batch(
    pool: &Pool,
    entries: &[(&str, &Value)],
    ttl_secs: Option<u64>,
) -> Result<usize> {
    if entries.is_empty() {
        return Ok(0);
    }

    let ttl = ttl_secs.unwrap_or(DEFAULT_TTL_SECS);
    let mut conn = pool.get().await.context("get redis connection")?;

    let mut pipe = redis::pipe();
    pipe.atomic();

    for (provider_id, payload) in entries {
        let key = keys::provider_payload_key(provider_id);
        let serialized =
            serde_json::to_string(payload).context("serialize provider payload in batch")?;
        pipe.cmd("SET")
            .arg(&key)
            .arg(&serialized)
            .arg("EX")
            .arg(ttl)
            .ignore();
    }

    pipe.query_async::<()>(&mut conn)
        .await
        .context("pipeline SET provider payloads")?;

    Ok(entries.len())
}

// ---------------------------------------------------------------------------
// Bulk Invalidation
// ---------------------------------------------------------------------------

/// Delete all cached provider payloads matching `gw:provider:payload:*` via SCAN + DEL.
/// Returns the total number of keys deleted.
pub async fn invalidate_all_provider_payloads(pool: &Pool) -> Result<u64> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let scan_pattern = format!("{}*", PROVIDER_PAYLOAD_PREFIX);
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
            .context("SCAN provider payloads")?;

        if !keys.is_empty() {
            let count: u64 = conn
                .del(keys.as_slice())
                .await
                .context("DEL provider payload keys")?;
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_provider_payload_key_prefix() {
        let k = keys::provider_payload_key("prov-123");
        assert!(k.starts_with("gw:provider:payload:"), "key was: {k}");
        assert!(k.contains("prov-123"));
    }

    #[test]
    fn test_default_ttl_constant() {
        assert_eq!(DEFAULT_TTL_SECS, 600);
    }

    #[test]
    fn test_payload_roundtrip_json() {
        let payload = json!({
            "apiKey": "sk-test",
            "baseUrl": "https://api.openai.com/v1",
            "provider": "openai",
        });
        let s = serde_json::to_string(&payload).unwrap();
        let back: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(back["apiKey"], payload["apiKey"]);
        assert_eq!(back["provider"], payload["provider"]);
    }

    // ---- Redis integration tests (ignored) ----

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_set_get_delete_provider_payload() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let provider_id = "test-provider-integration";
        let payload = json!({"apiKey": "sk-test", "provider": "openai"});

        set_cached_provider_payload(&pool, provider_id, &payload, Some(60))
            .await
            .unwrap();

        let got = get_cached_provider_payload(&pool, provider_id)
            .await
            .unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap()["apiKey"], "sk-test");

        delete_cached_provider_payload(&pool, provider_id)
            .await
            .unwrap();

        let gone = get_cached_provider_payload(&pool, provider_id)
            .await
            .unwrap();
        assert!(gone.is_none());
    }

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_sync_provider_payloads_batch() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let p1 = json!({"provider": "openai"});
        let p2 = json!({"provider": "anthropic"});
        let entries: Vec<(&str, &Value)> =
            vec![("batch-provider-1", &p1), ("batch-provider-2", &p2)];

        let synced = sync_provider_payloads_batch(&pool, &entries, Some(60))
            .await
            .unwrap();
        assert_eq!(synced, 2);

        let got1 = get_cached_provider_payload(&pool, "batch-provider-1")
            .await
            .unwrap();
        assert!(got1.is_some());
    }

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_invalidate_all_provider_payloads() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let payload = json!({"provider": "openai"});

        set_cached_provider_payload(&pool, "inv-provider-1", &payload, Some(60))
            .await
            .unwrap();

        let deleted = invalidate_all_provider_payloads(&pool).await.unwrap();
        assert!(deleted >= 1);
    }
}
