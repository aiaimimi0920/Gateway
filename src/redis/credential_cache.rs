use std::collections::HashMap;

use anyhow::{Context, Result};
use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::credential_runtime::{KeepaliveConfig, SessionAuthConfig};
use crate::routing::candidate::ProviderExecutionMode;

use super::keys;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    UserOwned,
    PlatformUnlimited,
    PlatformLimited,
    AccountCredential,
}

impl CredentialKind {
    /// Lower number = higher priority during resolution.
    fn priority(&self) -> u8 {
        match self {
            CredentialKind::UserOwned => 0,
            CredentialKind::AccountCredential => 1,
            CredentialKind::PlatformUnlimited => 2,
            CredentialKind::PlatformLimited => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    pub id: String,
    pub kind: CredentialKind,
    pub project_id: String,
    pub user_id: String,
    pub provider: String,

    // Credential payload
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub account_payload: Option<Value>,

    // Quota (platform-limited only)
    pub quota_total_tokens: Option<u64>,
    pub quota_remaining_tokens: Option<u64>,

    // Metadata
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// CredentialEntry helpers
// ---------------------------------------------------------------------------

impl CredentialEntry {
    /// Extract supported model list from account_payload.
    pub fn supported_models(&self) -> Vec<String> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("supported_models"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extract preset name from account_payload.
    pub fn preset_name(&self) -> Option<&str> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("preset"))
            .and_then(|v| v.as_str())
    }

    /// Extract extra_body from account_payload.
    pub fn extra_body_fields(&self) -> HashMap<String, Value> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("extra_body").or_else(|| p.get("extraBody")))
            .and_then(|v| serde_json::from_value::<HashMap<String, Value>>(v.clone()).ok())
            .unwrap_or_default()
    }

    /// Extract session-backed auth metadata from account_payload.
    pub fn session_auth(&self) -> Option<SessionAuthConfig> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("session_auth").or_else(|| p.get("sessionAuth")))
            .and_then(|v| serde_json::from_value::<SessionAuthConfig>(v.clone()).ok())
    }

    /// Extract keepalive steward metadata from account_payload.
    pub fn keepalive_config(&self) -> Option<KeepaliveConfig> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("keepalive"))
            .and_then(|v| serde_json::from_value::<KeepaliveConfig>(v.clone()).ok())
    }

    /// Extract runtime_state_object_key from account_payload.
    pub fn runtime_state_object_key(&self) -> Option<&str> {
        self.account_payload
            .as_ref()
            .and_then(|p| {
                p.get("runtime_state_object_key")
                    .or_else(|| p.get("runtimeStateObjectKey"))
            })
            .and_then(|v| v.as_str())
    }

    /// Extract optional human-readable account name from account_payload.
    pub fn account_name(&self) -> Option<&str> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("account_name").or_else(|| p.get("accountName")))
            .and_then(|v| v.as_str())
    }

    /// Extract provider execution_mode from account_payload.
    pub fn execution_mode(&self) -> Option<ProviderExecutionMode> {
        self.account_payload
            .as_ref()
            .and_then(|p| p.get("execution_mode").or_else(|| p.get("executionMode")))
            .and_then(|v| serde_json::from_value::<ProviderExecutionMode>(v.clone()).ok())
    }

    /// Extract endpoint execution-mode overrides from account_payload.
    pub fn endpoint_execution_modes(&self) -> Option<HashMap<String, ProviderExecutionMode>> {
        self.account_payload
            .as_ref()
            .and_then(|p| {
                p.get("endpoint_execution_modes")
                    .or_else(|| p.get("endpointExecutionModes"))
            })
            .and_then(|v| {
                serde_json::from_value::<HashMap<String, ProviderExecutionMode>>(v.clone()).ok()
            })
    }

    /// Check if this credential can serve a given model.
    /// An empty `supported_models` list means the credential supports any model.
    /// Glob patterns with trailing `*` are supported (prefix matching).
    pub fn supports_model(&self, model: &str) -> bool {
        let models = self.supported_models();
        if models.is_empty() {
            return true; // empty = supports any model
        }
        models.iter().any(|m| {
            if m.ends_with('*') {
                model.starts_with(m.trim_end_matches('*'))
            } else {
                m == model
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
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

fn current_timestamp_approx() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        1970 + now / (365 * 24 * 3600 * 1000),
        1,
        1,
        (now / 3600000) % 24,
        (now / 60000) % 60,
        (now / 1000) % 60,
        now % 1000,
    )
}

fn ensure_account_payload_object(
    entry: &mut CredentialEntry,
) -> &mut serde_json::Map<String, Value> {
    if !matches!(entry.account_payload, Some(Value::Object(_))) {
        entry.account_payload = Some(Value::Object(serde_json::Map::new()));
    }
    match entry.account_payload.as_mut() {
        Some(Value::Object(obj)) => obj,
        _ => unreachable!("account_payload object ensured"),
    }
}

fn merge_session_auth(
    current: Option<SessionAuthConfig>,
    patch: &SessionAuthConfig,
) -> SessionAuthConfig {
    let mut merged = current.unwrap_or_default();
    if !patch.transport.is_empty() {
        merged.transport = patch.transport.clone();
    }
    if patch.primary_cookie_name.is_some() {
        merged.primary_cookie_name = patch.primary_cookie_name.clone();
    }
    if patch.secondary_cookie_name.is_some() {
        merged.secondary_cookie_name = patch.secondary_cookie_name.clone();
    }
    if patch.header_name.is_some() {
        merged.header_name = patch.header_name.clone();
    }
    if patch.expires_at.is_some() {
        merged.expires_at = patch.expires_at.clone();
    }
    merged
}

fn merge_keepalive(current: Option<KeepaliveConfig>, patch: &KeepaliveConfig) -> KeepaliveConfig {
    let mut merged = current.unwrap_or_else(|| KeepaliveConfig {
        service_url: patch.service_url.clone(),
        ensure_path: None,
        auth_token: None,
        timeout_secs: None,
        refresh_before_secs: None,
    });
    if !patch.service_url.is_empty() {
        merged.service_url = patch.service_url.clone();
    }
    if patch.ensure_path.is_some() {
        merged.ensure_path = patch.ensure_path.clone();
    }
    if patch.auth_token.is_some() {
        merged.auth_token = patch.auth_token.clone();
    }
    if patch.timeout_secs.is_some() {
        merged.timeout_secs = patch.timeout_secs;
    }
    if patch.refresh_before_secs.is_some() {
        merged.refresh_before_secs = patch.refresh_before_secs;
    }
    merged
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
// OAuth token writeback
// ---------------------------------------------------------------------------

/// Write refreshed OAuth tokens back to a Redis credential entry.
///
/// Called by the background `token_refresh` task after a successful OAuth
/// refresh.  Updates `api_key` (the access token) and optionally the
/// `refresh_token` inside `account_payload`.  This ensures user-hosted
/// credentials survive DashMap cache eviction — next cache load from Redis
/// will pick up the fresh tokens.
///
/// Best-effort: returns `Ok(())` silently if the credential doesn't exist
/// in Redis (e.g. YAML-only credentials that have no Redis backing).
pub async fn write_back_refreshed_token(
    pool: &Pool,
    credential_id: &str,
    new_access_token: &str,
    new_refresh_token: Option<&str>,
) -> Result<()> {
    let existing = get_credential(pool, credential_id).await?;
    let Some(mut entry) = existing else {
        // Credential not in Redis (probably YAML-only). Nothing to write back.
        return Ok(());
    };

    entry.api_key = Some(new_access_token.to_string());

    if let Some(rt) = new_refresh_token {
        if let Some(ref mut payload) = entry.account_payload {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "refresh_token".to_string(),
                    serde_json::Value::String(rt.to_string()),
                );
            }
        }
    }

    // Update timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    entry.updated_at = format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        1970 + now / (365 * 24 * 3600 * 1000), // approximate year
        1,
        1,
        (now / 3600000) % 24,
        (now / 60000) % 60,
        (now / 1000) % 60,
        now % 1000,
    );

    set_credential(pool, &entry).await?;
    Ok(())
}

/// Write refreshed session/runtime material back to a Redis credential entry.
///
/// This is used by the keepalive steward for session-backed credentials.
/// Any field left as `None` is preserved from the existing credential.
pub async fn write_back_runtime_material(
    pool: &Pool,
    credential_id: &str,
    api_key: Option<&str>,
    headers: Option<&HashMap<String, String>>,
    extra_body: Option<&HashMap<String, Value>>,
    session_auth: Option<&SessionAuthConfig>,
    keepalive: Option<&KeepaliveConfig>,
    expires_at: Option<&str>,
    runtime_state_object_key: Option<&str>,
) -> Result<()> {
    let existing = get_credential(pool, credential_id).await?;
    let Some(mut entry) = existing else {
        return Ok(());
    };

    if let Some(key) = api_key {
        entry.api_key = Some(key.to_string());
    }

    if let Some(header_patch) = headers {
        let merged = entry.headers.get_or_insert_with(HashMap::new);
        for (k, v) in header_patch {
            merged.insert(k.clone(), v.clone());
        }
    }

    if let Some(extra_body_patch) = extra_body {
        let merged_extra_body = {
            let mut merged = entry.extra_body_fields();
            for (k, v) in extra_body_patch {
                merged.insert(k.clone(), v.clone());
            }
            serde_json::to_value(merged).context("serialize extra_body patch")?
        };
        ensure_account_payload_object(&mut entry)
            .insert("extra_body".to_string(), merged_extra_body);
    }

    if let Some(session_patch) = session_auth {
        let merged = merge_session_auth(entry.session_auth(), session_patch);
        ensure_account_payload_object(&mut entry).insert(
            "session_auth".to_string(),
            serde_json::to_value(merged).context("serialize session_auth patch")?,
        );
    }

    if let Some(keepalive_patch) = keepalive {
        let merged = merge_keepalive(entry.keepalive_config(), keepalive_patch);
        ensure_account_payload_object(&mut entry).insert(
            "keepalive".to_string(),
            serde_json::to_value(merged).context("serialize keepalive patch")?,
        );
    }

    if let Some(expiry) = expires_at {
        entry.expires_at = Some(expiry.to_string());
        let updated_session = entry.session_auth().map(|mut session| {
            session.expires_at = Some(expiry.to_string());
            session
        });
        if let Some(session) = updated_session {
            ensure_account_payload_object(&mut entry).insert(
                "session_auth".to_string(),
                serde_json::to_value(session).context("serialize expiry-updated session_auth")?,
            );
        }
    }

    if let Some(object_key) = runtime_state_object_key {
        ensure_account_payload_object(&mut entry).insert(
            "runtime_state_object_key".to_string(),
            Value::String(object_key.to_string()),
        );
    }

    entry.updated_at = current_timestamp_approx();
    set_credential(pool, &entry).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Credential checkout
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Credential affinity (sticky credential selection)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_ttl_seconds_none_when_no_expiry() {
        assert!(compute_ttl_seconds(None).is_none());
    }

    #[test]
    fn test_compute_ttl_seconds_past_returns_none() {
        // A date well in the past.
        assert!(compute_ttl_seconds(Some("2000-01-01T00:00:00.000Z")).is_none());
    }

    #[test]
    fn test_compute_ttl_seconds_future_returns_some() {
        // A date far in the future.
        let ttl = compute_ttl_seconds(Some("2099-01-01T00:00:00.000Z"));
        assert!(ttl.is_some());
        assert!(ttl.unwrap() > 0);
    }

    #[test]
    fn test_credential_kind_priority_ordering() {
        use CredentialKind::*;
        assert!(UserOwned.priority() < AccountCredential.priority());
        assert!(AccountCredential.priority() < PlatformUnlimited.priority());
        assert!(PlatformUnlimited.priority() < PlatformLimited.priority());
    }

    #[test]
    fn test_credential_entry_roundtrip_json() {
        let entry = CredentialEntry {
            id: "cred-1".to_string(),
            kind: CredentialKind::UserOwned,
            project_id: "proj-1".to_string(),
            user_id: "user-1".to_string(),
            provider: "openai".to_string(),
            api_key: Some("sk-test".to_string()),
            api_base_url: None,
            headers: None,
            account_payload: None,
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        };
        let s = serde_json::to_string(&entry).unwrap();
        let back: CredentialEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, entry.id);
        assert_eq!(back.kind, entry.kind);
        assert_eq!(back.api_key, entry.api_key);
    }

    // ── supports_model tests ──────────────────────────────────────────────

    fn make_entry_with_models(models: Vec<&str>) -> CredentialEntry {
        let payload = if models.is_empty() {
            None
        } else {
            Some(serde_json::json!({
                "supported_models": models,
            }))
        };
        CredentialEntry {
            id: "cred-m".to_string(),
            kind: CredentialKind::UserOwned,
            project_id: "proj-1".to_string(),
            user_id: "user-1".to_string(),
            provider: "openai".to_string(),
            api_key: Some("sk-test".to_string()),
            api_base_url: Some("https://api.openai.com".to_string()),
            headers: None,
            account_payload: payload,
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn supports_model_exact_match() {
        let entry = make_entry_with_models(vec!["gpt-4o", "gpt-4o-mini"]);
        assert!(entry.supports_model("gpt-4o"));
        assert!(entry.supports_model("gpt-4o-mini"));
        assert!(!entry.supports_model("claude-sonnet"));
    }

    #[test]
    fn supports_model_glob_match() {
        let entry = make_entry_with_models(vec!["gpt-*", "claude-*"]);
        assert!(entry.supports_model("gpt-4o"));
        assert!(entry.supports_model("gpt-5-codex"));
        assert!(entry.supports_model("claude-sonnet-4-6"));
        assert!(!entry.supports_model("llama-3"));
    }

    #[test]
    fn supports_model_empty_list_matches_any() {
        // No account_payload at all
        let entry = make_entry_with_models(vec![]);
        assert!(entry.supports_model("anything"));
        assert!(entry.supports_model("gpt-4o"));

        // account_payload with no supported_models key
        let mut entry2 = make_entry_with_models(vec![]);
        entry2.account_payload = Some(serde_json::json!({}));
        assert!(entry2.supports_model("anything"));
    }

    #[test]
    fn preset_name_extraction() {
        let entry = CredentialEntry {
            id: "c".to_string(),
            kind: CredentialKind::UserOwned,
            project_id: "p".to_string(),
            user_id: "u".to_string(),
            provider: "openai".to_string(),
            api_key: None,
            api_base_url: None,
            headers: None,
            account_payload: Some(serde_json::json!({"preset": "codex"})),
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        };
        assert_eq!(entry.preset_name(), Some("codex"));
    }

    #[test]
    fn extra_body_fields_extraction() {
        let entry = CredentialEntry {
            id: "c".to_string(),
            kind: CredentialKind::UserOwned,
            project_id: "p".to_string(),
            user_id: "u".to_string(),
            provider: "openai".to_string(),
            api_key: None,
            api_base_url: None,
            headers: None,
            account_payload: Some(serde_json::json!({
                "extra_body": {"store": false, "max_tokens": 1024}
            })),
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        };
        let eb = entry.extra_body_fields();
        assert_eq!(eb.get("store"), Some(&serde_json::json!(false)));
        assert_eq!(eb.get("max_tokens"), Some(&serde_json::json!(1024)));
    }

    #[test]
    fn session_auth_and_keepalive_extraction() {
        let entry = CredentialEntry {
            id: "c".to_string(),
            kind: CredentialKind::AccountCredential,
            project_id: "p".to_string(),
            user_id: "u".to_string(),
            provider: "grok".to_string(),
            api_key: Some("sso-token".to_string()),
            api_base_url: Some("https://grok.com".to_string()),
            headers: None,
            account_payload: Some(serde_json::json!({
                "session_auth": {
                    "transport": "cookie",
                    "primary_cookie_name": "sso",
                    "secondary_cookie_name": "sso-rw"
                },
                "keepalive": {
                    "service_url": "http://grok-keeper:8080",
                    "refresh_before_secs": 120
                }
            })),
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        };
        let session = entry.session_auth().expect("session_auth");
        assert_eq!(session.primary_cookie_name(), "sso");
        let keepalive = entry.keepalive_config().expect("keepalive");
        assert_eq!(keepalive.service_url, "http://grok-keeper:8080");
        assert_eq!(keepalive.refresh_before_secs(), 120);
    }

    #[test]
    fn account_payload_runtime_extraction_supports_platform_camel_case() {
        let entry = CredentialEntry {
            id: "platform-camel".to_string(),
            kind: CredentialKind::AccountCredential,
            project_id: "p".to_string(),
            user_id: "u".to_string(),
            provider: "chatgpt-web".to_string(),
            api_key: Some("session-token".to_string()),
            api_base_url: Some("https://chatgpt.com".to_string()),
            headers: None,
            account_payload: Some(serde_json::json!({
                "extraBody": {
                    "credentialMaterialKey": "chatgpt-session-main",
                    "appUrl": "https://chatgpt.com"
                },
                "sessionAuth": {
                    "transport": "bearer",
                    "headerName": "authorization"
                },
                "keepalive": {
                    "serviceUrl": "http://keeper:8080",
                    "refreshBeforeSecs": 120
                }
            })),
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        };

        let extra_body = entry.extra_body_fields();
        assert_eq!(
            extra_body.get("credentialMaterialKey"),
            Some(&serde_json::json!("chatgpt-session-main"))
        );
        let session = entry.session_auth().expect("session_auth");
        assert_eq!(session.transport, "bearer");
        assert_eq!(session.header_name(), Some("authorization"));
        let keepalive = entry.keepalive_config().expect("keepalive");
        assert_eq!(keepalive.service_url, "http://keeper:8080");
        assert_eq!(keepalive.refresh_before_secs(), 120);
    }

    #[test]
    fn browser_runtime_metadata_extraction_supports_snake_and_camel_case() {
        let entry = CredentialEntry {
            id: "canvas-cred".to_string(),
            kind: CredentialKind::AccountCredential,
            project_id: "p".to_string(),
            user_id: "u".to_string(),
            provider: "gemini-canvas".to_string(),
            api_key: Some(String::new()),
            api_base_url: Some("https://gemini.google.com".to_string()),
            headers: None,
            account_payload: Some(serde_json::json!({
                "runtimeStateObjectKey": "objects/gemini-canvas/auth-1.json",
                "account_name": "canvas-main"
            })),
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        };

        assert_eq!(
            entry.runtime_state_object_key(),
            Some("objects/gemini-canvas/auth-1.json")
        );
        assert_eq!(entry.account_name(), Some("canvas-main"));
    }

    #[tokio::test]
    #[ignore = "requires live Redis"]
    async fn test_set_get_delete_credential() {
        let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
        let entry = CredentialEntry {
            id: "test-cred-integration".to_string(),
            kind: CredentialKind::UserOwned,
            project_id: "proj-x".to_string(),
            user_id: "user-x".to_string(),
            provider: "openai".to_string(),
            api_key: Some("sk-test".to_string()),
            api_base_url: None,
            headers: None,
            account_payload: None,
            quota_total_tokens: None,
            quota_remaining_tokens: None,
            expires_at: None,
            created_at: "2024-01-01T00:00:00.000Z".to_string(),
            updated_at: "2024-01-01T00:00:00.000Z".to_string(),
        };
        set_credential(&pool, &entry).await.unwrap();
        let got = get_credential(&pool, &entry.id).await.unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().id, entry.id);
        delete_credential(&pool, &entry.id).await.unwrap();
        let gone = get_credential(&pool, &entry.id).await.unwrap();
        assert!(gone.is_none());
    }
}
