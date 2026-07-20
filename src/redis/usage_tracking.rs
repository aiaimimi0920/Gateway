use anyhow::{Context, Result};
use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::protocol::canonical::TokenUsage;

use super::keys;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub request_id: String,
    pub credential_id: String,
    pub project_id: String,
    pub user_id: String,
    pub model: String,
    pub provider: String,

    // Actual usage from upstream response
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,

    // Timing
    pub request_started_at: String,
    pub request_completed_at: String,
    pub latency_ms: u64,

    // Status
    pub success: bool,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCheckResult {
    pub allowed: bool,
    pub remaining_tokens: i64,
    /// e.g. `"quota_exceeded"`, `"quota_not_initialized"`, `"quota_corrupt"`
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Token parsing
// ---------------------------------------------------------------------------

/// Extract token usage from an upstream AI provider response body.
///
/// Supports:
/// - OpenAI-compatible: `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`
/// - Anthropic: `usage.input_tokens`, `usage.output_tokens`,
///   `usage.cache_creation_input_tokens`, `usage.cache_read_input_tokens`
///
/// Returns `None` when the response does not contain recognisable usage data.
pub fn parse_upstream_usage(body: &Value, _provider: &str) -> Option<TokenUsage> {
    let usage = body.get("usage")?;

    let cache_creation_input_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|value| value.as_u64());
    let cache_read_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|details| details.get("cached_tokens"))
                .and_then(|tokens| tokens.as_u64())
        })
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .and_then(|details| details.get("cached_tokens"))
                .and_then(|tokens| tokens.as_u64())
        });

    if let (Some(input), Some(output)) = (
        usage.get("input_tokens").and_then(|value| value.as_u64()),
        usage.get("output_tokens").and_then(|value| value.as_u64()),
    ) {
        let total = usage
            .get("total_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(input + output);
        return Some(TokenUsage {
            prompt_tokens: input,
            completion_tokens: output,
            total_tokens: total,
            cache_creation_input_tokens,
            cache_read_input_tokens,
        });
    }

    let prompt = usage.get("prompt_tokens")?.as_u64()?;
    let completion = usage.get("completion_tokens")?.as_u64()?;
    let total = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(prompt + completion);

    Some(TokenUsage {
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: total,
        cache_creation_input_tokens,
        cache_read_input_tokens,
    })
}

/// Rough token count estimation: ~4 characters per token (ceiling division).
pub fn estimate_token_count(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    ((text.len() as u64) + 3) / 4
}

// ---------------------------------------------------------------------------
// Quota management
// ---------------------------------------------------------------------------

/// Read-only quota check. Returns whether the credential has enough quota.
pub async fn check_quota(
    pool: &Pool,
    credential_id: &str,
    estimated_tokens: u64,
) -> Result<QuotaCheckResult> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let key = keys::quota_key(credential_id);

    let raw: Option<String> = conn.get(&key).await.context("GET quota")?;

    let raw = match raw {
        None => {
            return Ok(QuotaCheckResult {
                allowed: false,
                remaining_tokens: 0,
                reason: Some("quota_not_initialized".to_string()),
            })
        }
        Some(s) => s,
    };

    let remaining: i64 = match raw.parse() {
        Ok(v) => v,
        Err(_) => {
            return Ok(QuotaCheckResult {
                allowed: false,
                remaining_tokens: 0,
                reason: Some("quota_corrupt".to_string()),
            })
        }
    };

    if remaining < estimated_tokens as i64 {
        return Ok(QuotaCheckResult {
            allowed: false,
            remaining_tokens: remaining,
            reason: Some("quota_exceeded".to_string()),
        });
    }

    Ok(QuotaCheckResult {
        allowed: true,
        remaining_tokens: remaining,
        reason: None,
    })
}

/// Atomically deduct `tokens` from a credential's quota counter (DECRBY).
/// Returns the new remaining value (may be negative).
pub async fn deduct_quota(pool: &Pool, credential_id: &str, tokens: u64) -> Result<i64> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let remaining: i64 = conn
        .decr(keys::quota_key(credential_id), tokens as i64)
        .await
        .context("DECRBY quota")?;
    Ok(remaining)
}

/// Atomically refund tokens back into a credential quota counter (INCRBY).
/// Returns the new remaining value after refund.
pub async fn refund_quota(pool: &Pool, credential_id: &str, tokens: u64) -> Result<i64> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let remaining: i64 = conn
        .incr(keys::quota_key(credential_id), tokens as i64)
        .await
        .context("INCRBY quota")?;
    Ok(remaining)
}

/// Reconcile a previous pre-deduct against the actual upstream usage.
///
/// - when `actual_total_tokens > pre_deducted_tokens`, deduct the delta
/// - when `actual_total_tokens < pre_deducted_tokens`, refund the delta
/// - when equal, no-op
pub async fn settle_quota_after_usage(
    pool: &Pool,
    credential_id: &str,
    pre_deducted_tokens: u64,
    actual_total_tokens: u64,
) -> Result<()> {
    if actual_total_tokens > pre_deducted_tokens {
        let delta = actual_total_tokens - pre_deducted_tokens;
        deduct_quota(pool, credential_id, delta).await?;
    } else if pre_deducted_tokens > actual_total_tokens {
        let delta = pre_deducted_tokens - actual_total_tokens;
        refund_quota(pool, credential_id, delta).await?;
    }
    Ok(())
}

/// Set (or reset) a credential's quota counter. Optionally set a TTL.
pub async fn init_quota(
    pool: &Pool,
    credential_id: &str,
    total_tokens: u64,
    ttl_secs: Option<u64>,
) -> Result<()> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let key = keys::quota_key(credential_id);

    match ttl_secs {
        Some(ttl) if ttl > 0 => {
            redis::cmd("SET")
                .arg(&key)
                .arg(total_tokens)
                .arg("EX")
                .arg(ttl)
                .query_async::<()>(&mut conn)
                .await
                .context("SET quota with EX")?;
        }
        _ => {
            let _: () = conn.set(&key, total_tokens).await.context("SET quota")?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Usage report queue
// ---------------------------------------------------------------------------

/// Push a usage report onto the Redis list for async consumption.
pub async fn enqueue_usage_report(pool: &Pool, report: &UsageReport) -> Result<()> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let payload = serde_json::to_string(report).context("serialize UsageReport")?;
    let _: i64 = conn
        .rpush(keys::usage_reports_key(), payload)
        .await
        .context("RPUSH usage report")?;
    Ok(())
}

/// Pop up to `batch_size` usage reports from the queue (LPOP).
pub async fn dequeue_usage_reports(pool: &Pool, batch_size: usize) -> Result<Vec<UsageReport>> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let key = keys::usage_reports_key();
    let mut results = Vec::with_capacity(batch_size);

    for _ in 0..batch_size {
        let raw: Option<String> = conn.lpop(&key, None).await.context("LPOP usage report")?;
        match raw {
            None => break,
            Some(s) => {
                let report: UsageReport =
                    serde_json::from_str(&s).context("deserialize UsageReport")?;
                results.push(report);
            }
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- parse_upstream_usage ----

    #[test]
    fn test_parse_openai_usage() {
        let body = json!({
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        });
        let usage = parse_upstream_usage(&body, "openai").unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_parse_openai_usage_infers_total() {
        let body = json!({
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 7
            }
        });
        let usage = parse_upstream_usage(&body, "openai").unwrap();
        assert_eq!(usage.total_tokens, 12);
    }

    #[test]
    fn test_parse_openai_cached_tokens() {
        let body = json!({
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 3,
                "total_tokens": 15,
                "prompt_tokens_details": {
                    "cached_tokens": 6
                }
            }
        });
        let usage = parse_upstream_usage(&body, "openai").unwrap();
        assert_eq!(usage.cache_read_input_tokens, Some(6));
    }

    #[test]
    fn test_parse_responses_usage_shape() {
        let body = json!({
            "usage": {
                "input_tokens": 14,
                "output_tokens": 9,
                "total_tokens": 23,
                "input_tokens_details": {
                    "cached_tokens": 5
                }
            }
        });
        let usage = parse_upstream_usage(&body, "openai").unwrap();
        assert_eq!(usage.prompt_tokens, 14);
        assert_eq!(usage.completion_tokens, 9);
        assert_eq!(usage.total_tokens, 23);
        assert_eq!(usage.cache_read_input_tokens, Some(5));
    }

    #[test]
    fn test_parse_anthropic_usage() {
        let body = json!({
            "usage": {
                "input_tokens": 15,
                "output_tokens": 25,
                "cache_creation_input_tokens": 200,
                "cache_read_input_tokens": 120
            }
        });
        let usage = parse_upstream_usage(&body, "anthropic").unwrap();
        assert_eq!(usage.prompt_tokens, 15);
        assert_eq!(usage.completion_tokens, 25);
        assert_eq!(usage.total_tokens, 40);
        assert_eq!(usage.cache_creation_input_tokens, Some(200));
        assert_eq!(usage.cache_read_input_tokens, Some(120));
    }

    #[test]
    fn test_parse_anthropic_case_insensitive() {
        let body = json!({
            "usage": { "input_tokens": 3, "output_tokens": 4 }
        });
        let usage = parse_upstream_usage(&body, "  Anthropic  ").unwrap();
        assert_eq!(usage.total_tokens, 7);
    }

    #[test]
    fn test_parse_upstream_usage_missing_returns_none() {
        let body = json!({ "choices": [] });
        assert!(parse_upstream_usage(&body, "openai").is_none());
    }

    #[test]
    fn test_parse_upstream_usage_null_body() {
        let body = json!(null);
        assert!(parse_upstream_usage(&body, "openai").is_none());
    }

    // ---- estimate_token_count ----

    #[test]
    fn test_estimate_token_count_empty() {
        assert_eq!(estimate_token_count(""), 0);
    }

    #[test]
    fn test_estimate_token_count_four_chars() {
        assert_eq!(estimate_token_count("abcd"), 1);
    }

    #[test]
    fn test_estimate_token_count_ceil() {
        // 5 chars → ceil(5/4) = 2
        assert_eq!(estimate_token_count("abcde"), 2);
    }

    #[test]
    fn test_estimate_token_count_larger() {
        // 100 chars → 25
        let text = "a".repeat(100);
        assert_eq!(estimate_token_count(&text), 25);
    }

    // ---- Redis integration tests (ignored) ----

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_init_check_deduct_quota() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let cid = "test-quota-integration";

        init_quota(&pool, cid, 1000, None).await.unwrap();

        let check = check_quota(&pool, cid, 100).await.unwrap();
        assert!(check.allowed);
        assert_eq!(check.remaining_tokens, 1000);

        let after = deduct_quota(&pool, cid, 100).await.unwrap();
        assert_eq!(after, 900);

        let check2 = check_quota(&pool, cid, 1000).await.unwrap();
        assert!(!check2.allowed);
        assert_eq!(check2.reason.unwrap(), "quota_exceeded");
    }

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_refund_quota() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let cid = "test-refund-integration";
        init_quota(&pool, cid, 1000, None).await.unwrap();
        deduct_quota(&pool, cid, 300).await.unwrap();
        let remaining = refund_quota(&pool, cid, 100).await.unwrap();
        assert_eq!(remaining, 800);
    }

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_settle_quota_after_usage() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let cid = "test-settle-integration";
        init_quota(&pool, cid, 1000, None).await.unwrap();
        deduct_quota(&pool, cid, 300).await.unwrap();
        settle_quota_after_usage(&pool, cid, 300, 200)
            .await
            .unwrap();
        let check = check_quota(&pool, cid, 1).await.unwrap();
        assert_eq!(check.remaining_tokens, 800);
    }

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_enqueue_dequeue_usage_report() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let report = UsageReport {
            request_id: "req-1".to_string(),
            credential_id: "cred-1".to_string(),
            project_id: "proj-1".to_string(),
            user_id: "user-1".to_string(),
            model: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            request_started_at: "2024-01-01T00:00:00.000Z".to_string(),
            request_completed_at: "2024-01-01T00:00:01.000Z".to_string(),
            latency_ms: 1000,
            success: true,
            error_code: None,
        };
        enqueue_usage_report(&pool, &report).await.unwrap();
        let batch = dequeue_usage_reports(&pool, 10).await.unwrap();
        assert!(!batch.is_empty());
        let got = batch.iter().find(|r| r.request_id == "req-1");
        assert!(got.is_some());
    }
}
