//! Credential checkout, affinity and request selection.
use super::{get_credential, list_by_project, CredentialEntry, CredentialKind};
use crate::redis::keys;
use anyhow::{Context, Result};
use deadpool_redis::Pool;
use redis::AsyncCommands;

/// Checkout a credential for "unlimited refill" mode.
/// Returns a credential the user can use locally (not for relay).
///
/// Strategy: round-robin across available platform credentials for the
/// requested provider/model. Uses the credential's `quota_remaining_tokens`
/// as a proxy for checkout count to balance load (lowest usage first).
pub async fn checkout_credential(
    pool: &Pool,
    project_id: &str,
    _user_id: &str,
    provider: Option<&str>,
    model: Option<&str>,
) -> Result<Option<CredentialEntry>> {
    let all = list_by_project(pool, project_id).await?;

    let mut candidates: Vec<CredentialEntry> = all
        .into_iter()
        .filter(|c| {
            // Only PlatformUnlimited credentials for checkout
            if c.kind != CredentialKind::PlatformUnlimited {
                return false;
            }
            // Filter by provider if specified
            if let Some(prov) = provider {
                if c.provider != prov {
                    return false;
                }
            }
            // Filter by model if specified
            if let Some(m) = model {
                if !c.supports_model(m) {
                    return false;
                }
            }
            true
        })
        .collect();

    if candidates.is_empty() {
        return Ok(None);
    }

    // Sort by checkout count (lowest first) for load balancing.
    // We use quota_remaining_tokens as a proxy — lower = more used = lower priority.
    // If not set, treat as 0 (highest priority / least used).
    candidates.sort_by_key(|c| std::cmp::Reverse(c.quota_remaining_tokens.unwrap_or(u64::MAX)));

    // Atomically increment the checkout counter for the selected credential.
    let selected = &candidates[0];
    let mut conn = pool.get().await.context("get redis connection")?;
    let counter_key = format!("gw:checkout:count:{}", selected.id);
    let _: () = conn.incr(&counter_key, 1i64).await.unwrap_or(());

    Ok(Some(selected.clone()))
}

/// Default affinity TTL: 1 hour. After this, affinity is cleared and a new
/// credential can be selected.
const AFFINITY_TTL_SECS: u64 = 3600;

/// Get the affinity credential ID for a given scope + model.
///
/// `affinity_scope` is typically:
/// - `"session:{session_key}"` for conversation-level affinity
/// - `"user:{user_id}"` for user-level affinity
pub async fn get_credential_affinity(
    pool: &Pool,
    affinity_scope: &str,
    model: &str,
) -> Result<Option<String>> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let key = keys::credential_affinity_key(affinity_scope, model);
    let id: Option<String> = conn.get(&key).await.unwrap_or(None);
    Ok(id)
}

/// Set the affinity credential ID for a given scope + model.
/// TTL ensures stale affinities are cleaned up automatically.
pub async fn set_credential_affinity(
    pool: &Pool,
    affinity_scope: &str,
    model: &str,
    credential_id: &str,
) -> Result<()> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let key = keys::credential_affinity_key(affinity_scope, model);
    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg(credential_id)
        .arg("EX")
        .arg(AFFINITY_TTL_SECS)
        .query_async(&mut conn)
        .await
        .context("SET credential affinity")?;
    Ok(())
}

/// Smart credential resolution for an incoming gateway request.
///
/// Resolution strategy:
/// 1. If `preferred_id` is given, fetch it directly and verify project/user.
/// 2. Otherwise enumerate the project's credentials and find the best match.
///
/// Priority: user-owned > account-credential > platform-unlimited > platform-limited.
/// For `platform-limited`, higher remaining quota wins.
pub async fn resolve_credential_for_request(
    pool: &Pool,
    project_id: &str,
    user_id: &str,
    preferred_id: Option<&str>,
    provider: Option<&str>,
) -> Result<Option<CredentialEntry>> {
    // Fast path: preferred credential.
    if let Some(pid) = preferred_id {
        if let Some(entry) = get_credential(pool, pid).await? {
            if entry.project_id == project_id && entry.user_id == user_id {
                if let Some(prov) = provider {
                    if entry.provider != prov {
                        return Ok(None);
                    }
                }
                return Ok(Some(entry));
            }
        }
        // preferred not found or mismatch — fall through.
    }

    // Enumerate and filter.
    let mut candidates: Vec<CredentialEntry> = list_by_project(pool, project_id)
        .await?
        .into_iter()
        .filter(|e| {
            if e.user_id != user_id {
                return false;
            }
            if let Some(prov) = provider {
                if e.provider != prov {
                    return false;
                }
            }
            true
        })
        .collect();

    if candidates.is_empty() {
        return Ok(None);
    }

    candidates.sort_by(|a, b| {
        let pa = a.kind.priority();
        let pb = b.kind.priority();
        if pa != pb {
            return pa.cmp(&pb);
        }
        // For platform-limited, prefer higher remaining quota.
        if a.kind == CredentialKind::PlatformLimited && b.kind == CredentialKind::PlatformLimited {
            let ra = a.quota_remaining_tokens.unwrap_or(0);
            let rb = b.quota_remaining_tokens.unwrap_or(0);
            return rb.cmp(&ra); // descending
        }
        std::cmp::Ordering::Equal
    });

    Ok(candidates.into_iter().next())
}
