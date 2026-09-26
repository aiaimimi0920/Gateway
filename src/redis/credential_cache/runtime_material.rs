//! Persist refreshed tokens and session runtime material.
use super::{get_credential, set_credential, CredentialEntry};
use crate::credential_runtime::{KeepaliveConfig, SessionAuthConfig};
use anyhow::{Context, Result};
use deadpool_redis::Pool;
use serde_json::Value;
use std::collections::HashMap;

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
