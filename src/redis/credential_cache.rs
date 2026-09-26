//! Redis credential persistence, indexes and model lookups.

use anyhow::{Context, Result};
use deadpool_redis::Pool;
use redis::AsyncCommands;

use super::keys;

mod entry;
mod runtime_material;
mod selection;

pub use entry::{CredentialEntry, CredentialKind};
pub use runtime_material::{write_back_refreshed_token, write_back_runtime_material};
pub use selection::{
    checkout_credential, get_credential_affinity, resolve_credential_for_request,
    set_credential_affinity,
};

// ---------------------------------------------------------------------------
// Expiry parsing
// ---------------------------------------------------------------------------

/// Compute TTL in seconds from an ISO-8601 `expires_at` string.
/// Returns `None` when there is no expiry or the expiry is already in the past.
fn compute_ttl_seconds(expires_at: Option<&str>) -> Option<u64> {
    let expires_at = expires_at?;
    let expires_ms = chrono_or_manual_parse(expires_at)?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i128;
    let diff_secs = (expires_ms - now_ms) / 1000;
    if diff_secs > 0 {
        Some(diff_secs as u64)
    } else {
        None
    }
}

/// Minimal ISO-8601 UTC timestamp parser (avoids pulling in `chrono`).
/// Handles the common format produced by JS `new Date().toISOString()`:
/// `"2025-12-31T23:59:59.999Z"`.
fn chrono_or_manual_parse(s: &str) -> Option<i128> {
    // Use the `time` crate if available; otherwise fall back to a simple approach.
    // Since we only have `sha2`/`hex`/`anyhow` in Cargo.toml, we implement a
    // lightweight parser for the RFC-3339 subset that JS emits.
    let s = s.trim().trim_end_matches('Z');
    // "2025-12-31T23:59:59.999" or "2025-12-31T23:59:59"
    let (date_part, time_part) = s.split_once('T')?;
    let mut date_parts = date_part.splitn(3, '-');
    let year: i128 = date_parts.next()?.parse().ok()?;
    let month: i128 = date_parts.next()?.parse().ok()?;
    let day: i128 = date_parts.next()?.parse().ok()?;

    let time_no_frac = time_part.split('.').next().unwrap_or(time_part);
    let mut time_parts = time_no_frac.splitn(3, ':');
    let hour: i128 = time_parts.next()?.parse().ok()?;
    let minute: i128 = time_parts.next()?.parse().ok()?;
    let second: i128 = time_parts.next().unwrap_or("0").parse().ok()?;

    // Days from epoch (1970-01-01) using the proleptic Gregorian formula.
    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 { month + 9 } else { month - 3 };
    let days = 365 * y + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + day - 719_469;

    let ms = (days * 86_400 + hour * 3_600 + minute * 60 + second) * 1_000;
    Some(ms)
}

// ---------------------------------------------------------------------------
// Public API — CRUD
// ---------------------------------------------------------------------------

/// Read a single credential by ID.
pub async fn get_credential(pool: &Pool, credential_id: &str) -> Result<Option<CredentialEntry>> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let raw: Option<String> = conn
        .get(keys::credential_key(credential_id))
        .await
        .context("GET credential")?;
    match raw {
        None => Ok(None),
        Some(s) => {
            let entry: CredentialEntry =
                serde_json::from_str(&s).context("deserialize CredentialEntry")?;
            Ok(Some(entry))
        }
    }
}

/// Write a credential to Redis and maintain project/user set indexes.
pub async fn set_credential(pool: &Pool, entry: &CredentialEntry) -> Result<()> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let key = keys::credential_key(&entry.id);
    let payload = serde_json::to_string(entry).context("serialize CredentialEntry")?;
    let ttl = compute_ttl_seconds(entry.expires_at.as_deref());

    let mut pipe = redis::pipe();
    pipe.atomic();

    match ttl {
        Some(secs) => {
            pipe.cmd("SET")
                .arg(&key)
                .arg(&payload)
                .arg("EX")
                .arg(secs)
                .ignore();
        }
        None => {
            pipe.cmd("SET").arg(&key).arg(&payload).ignore();
        }
    }

    pipe.sadd(
        keys::credential_project_index_key(&entry.project_id),
        &entry.id,
    )
    .ignore();
    pipe.sadd(keys::credential_user_index_key(&entry.user_id), &entry.id)
        .ignore();

    // Maintain model-based indexes for fast per-model lookups.
    let models = entry.supported_models();
    for model in &models {
        pipe.sadd(keys::credential_model_index_key(model), &entry.id)
            .ignore();
    }

    // Bump the global credential version so the Tier-1 in-memory cache
    // notices the mutation and reloads fresh entries from Redis.
    pipe.cmd("INCR")
        .arg(keys::credential_version_key())
        .ignore();

    pipe.query_async::<()>(&mut conn)
        .await
        .context("pipeline SET + SADD credential")?;
    Ok(())
}

/// Remove a credential and clean up set indexes.
pub async fn delete_credential(pool: &Pool, credential_id: &str) -> Result<()> {
    // Read first so we know which indexes to clean.
    let existing = get_credential(pool, credential_id).await?;

    let mut conn = pool.get().await.context("get redis connection")?;
    let mut pipe = redis::pipe();
    pipe.atomic();

    pipe.del(keys::credential_key(credential_id)).ignore();

    if let Some(ref e) = existing {
        pipe.srem(
            keys::credential_project_index_key(&e.project_id),
            credential_id,
        )
        .ignore();
        pipe.srem(keys::credential_user_index_key(&e.user_id), credential_id)
            .ignore();

        // Clean model-based indexes.
        let models = e.supported_models();
        for model in &models {
            pipe.srem(keys::credential_model_index_key(model), credential_id)
                .ignore();
        }
    }

    pipe.cmd("INCR")
        .arg(keys::credential_version_key())
        .ignore();

    pipe.query_async::<()>(&mut conn)
        .await
        .context("pipeline DEL + SREM credential")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// List helpers
// ---------------------------------------------------------------------------

/// List all credentials in a project, accessible for credential-based routing.
pub async fn list_by_project(pool: &Pool, project_id: &str) -> Result<Vec<CredentialEntry>> {
    let mut conn = pool.get().await.context("get redis connection")?;
    let ids: Vec<String> = conn
        .smembers(keys::credential_project_index_key(project_id))
        .await
        .context("SMEMBERS project index")?;

    if ids.is_empty() {
        return Ok(vec![]);
    }

    let raw_keys: Vec<String> = ids.iter().map(|id| keys::credential_key(id)).collect();
    let raw_values: Vec<Option<String>> = redis::cmd("MGET")
        .arg(&raw_keys)
        .query_async(&mut conn)
        .await
        .context("MGET credentials")?;

    let mut entries = Vec::with_capacity(ids.len());
    let mut stale: Vec<&str> = Vec::new();

    for (i, raw) in raw_values.iter().enumerate() {
        match raw {
            Some(s) => {
                if let Ok(e) = serde_json::from_str::<CredentialEntry>(s) {
                    entries.push(e);
                } else {
                    stale.push(&ids[i]);
                }
            }
            None => stale.push(&ids[i]),
        }
    }

    // Lazily clean stale index entries.
    if !stale.is_empty() {
        let _: () = conn
            .srem(
                keys::credential_project_index_key(project_id),
                stale.as_slice(),
            )
            .await
            .unwrap_or(());
    }

    Ok(entries)
}

/// List all credentials in a project that can serve a given model,
/// with proper user isolation.
///
/// - `UserOwned` and `AccountCredential` are only returned if they
///   belong to the specified `user_id`.
/// - `PlatformUnlimited` and `PlatformLimited` are shared across all
///   users in the project and always returned if they match the model.
/// - If `user_id` is `None`, only platform credentials are returned.
pub async fn list_credentials_for_model(
    pool: &Pool,
    project_id: &str,
    model: &str,
    user_id: Option<&str>,
) -> Result<Vec<CredentialEntry>> {
    let all = list_by_project(pool, project_id).await?;
    Ok(all
        .into_iter()
        .filter(|c| {
            if !c.supports_model(model) {
                return false;
            }
            match c.kind {
                // User-scoped credentials: only visible to their owner
                CredentialKind::UserOwned | CredentialKind::AccountCredential => {
                    match user_id {
                        Some(uid) => c.user_id == uid,
                        None => false, // anonymous requests can't use personal credentials
                    }
                }
                // Platform credentials: shared, visible to everyone in the project
                CredentialKind::PlatformUnlimited | CredentialKind::PlatformLimited => true,
            }
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Fast model-based lookup (Tier 2)
// ---------------------------------------------------------------------------

/// Fast credential lookup by model using the model index.
/// Falls back to project-based scan if the model index yields no results.
///
/// **User isolation:**
/// - `UserOwned` / `AccountCredential`: only returned when `user_id` matches.
/// - `PlatformUnlimited` / `PlatformLimited`: always returned.
pub async fn lookup_credentials_by_model(
    pool: &Pool,
    model: &str,
    user_id: Option<&str>,
) -> Result<Vec<CredentialEntry>> {
    let mut conn = pool.get().await.context("get redis connection")?;

    // Step 1: Try exact model index.
    let ids: Vec<String> = conn
        .smembers(keys::credential_model_index_key(model))
        .await
        .context("SMEMBERS model index")?;

    let entries = if !ids.is_empty() {
        fetch_and_filter_credentials(&mut conn, &ids, model, user_id).await?
    } else {
        // Step 2: Try glob-matching model indexes.
        //
        // Credentials may register patterns like "gpt-*" or "claude-*".
        // We scan for model index keys that could match. This is bounded by
        // the number of distinct model patterns (typically < 20), not by the
        // number of credentials.
        let pattern = format!("gw:cred:model:*");
        let mut cursor: u64 = 0;
        let mut matched_ids: Vec<String> = Vec::new();

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .context("SCAN model index keys")?;

            for key in &keys {
                // Extract the model pattern from the key: "gw:cred:model:{pattern}"
                if let Some(key_model) = key.strip_prefix("gw:cred:model:") {
                    // Check if this pattern matches the requested model
                    let matches = if key_model.ends_with('*') {
                        model.starts_with(key_model.trim_end_matches('*'))
                    } else {
                        key_model == model
                    };
                    if matches {
                        let ids: Vec<String> = conn.smembers(key).await.unwrap_or_default();
                        matched_ids.extend(ids);
                    }
                }
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        if !matched_ids.is_empty() {
            // Deduplicate
            matched_ids.sort();
            matched_ids.dedup();
            fetch_and_filter_credentials(&mut conn, &matched_ids, model, user_id).await?
        } else {
            Vec::new()
        }
    };

    Ok(entries)
}

/// Helper: MGET credential IDs, deserialize, and apply model + user isolation filters.
async fn fetch_and_filter_credentials(
    conn: &mut deadpool_redis::Connection,
    ids: &[String],
    model: &str,
    user_id: Option<&str>,
) -> Result<Vec<CredentialEntry>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let raw_keys: Vec<String> = ids.iter().map(|id| keys::credential_key(id)).collect();
    let raw_values: Vec<Option<String>> = redis::cmd("MGET")
        .arg(&raw_keys)
        .query_async(conn)
        .await
        .context("MGET credentials by model")?;

    let mut entries = Vec::with_capacity(ids.len());
    for raw in &raw_values {
        if let Some(s) = raw {
            if let Ok(e) = serde_json::from_str::<CredentialEntry>(s) {
                // Apply model filter (double-check, since glob indexes can be broad)
                if !e.supports_model(model) {
                    continue;
                }
                // Apply user isolation
                match e.kind {
                    CredentialKind::UserOwned | CredentialKind::AccountCredential => {
                        match user_id {
                            Some(uid) if e.user_id == uid => entries.push(e),
                            _ => {} // skip: not this user's credential
                        }
                    }
                    CredentialKind::PlatformUnlimited | CredentialKind::PlatformLimited => {
                        entries.push(e);
                    }
                }
            }
        }
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
