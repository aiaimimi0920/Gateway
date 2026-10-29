//! Server cache updates compare the original material and CAS the complete raw entry.
use super::transport::TokenResponse;
use redis::AsyncCommands;
use serde_json::{json, Value};

const CAS: &str = r#"
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return 0 end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
return 1
"#;

fn patch(
    raw: &str,
    access: &str,
    refresh: &str,
    token: &TokenResponse,
    expiry: &str,
) -> Option<String> {
    let mut entry: Value = serde_json::from_str(raw).ok()?;
    if entry.get("api_key")?.as_str()? != access {
        return None;
    }
    let rt = entry
        .pointer("/account_payload/refresh_token")
        .and_then(Value::as_str);
    if rt.is_some_and(|rt| rt != refresh) {
        return None;
    }
    entry["api_key"] = json!(token.access_token);
    entry["expires_at"] = json!(expiry);
    entry["updated_at"] = json!(crate::routing::config::future_rfc3339_after_secs(0));
    if !entry.get("account_payload").is_some_and(Value::is_object) {
        entry["account_payload"] = json!({});
    }
    let payload = &mut entry["account_payload"];
    payload["api_key"] = json!(token.access_token);
    payload["expires_at"] = json!(expiry);
    payload["token_expires_in_secs"] = json!(token.expires_in);
    if let Some(rt) = &token.refresh_token {
        payload["refresh_token"] = json!(rt);
    }
    for name in ["session_auth", "sessionAuth"] {
        if payload.get(name).is_some_and(Value::is_object) {
            payload[name]["expires_at"] = json!(expiry);
        }
    }
    serde_json::to_string(&entry).ok()
}

pub(super) async fn persist(
    pool: &deadpool_redis::Pool,
    id: &str,
    expected_access: &str,
    expected_refresh: &str,
    token: &TokenResponse,
    expiry: &str,
) -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut connection = pool.get().await?;
        let key = crate::redis::keys::credential_key(id);
        for _ in 0..3 {
            let raw: Option<String> = connection.get(&key).await?;
            let Some(raw) = raw else {
                return Ok(());
            };
            let Some(updated) = patch(&raw, expected_access, expected_refresh, token, expiry)
            else {
                return Ok(()); // Removed or reauthorized: never restore retired material.
            };
            let ttl = time::OffsetDateTime::parse(
                expiry,
                &time::format_description::well_known::Rfc3339,
            )?
            .unix_timestamp()
            .saturating_sub(time::OffsetDateTime::now_utc().unix_timestamp())
            .max(1);
            let applied: i32 = redis::Script::new(CAS)
                .key(&key)
                .arg(&raw)
                .arg(updated)
                .arg(ttl)
                .invoke_async(&mut connection)
                .await?;
            if applied == 1 {
                return Ok(());
            }
        }
        anyhow::bail!("OAuth cache revision changed during writeback")
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_patch_preserves_quotas_and_rejects_reauthorization() {
        let raw = json!({"api_key":"old", "quota_remaining_tokens":42,
            "unknown_field":"retained", "account_payload":{"refresh_token":"old-rt",
            "session_auth":{"expires_at":"old"}, "headers":{"x-id":"account"}}})
        .to_string();
        let token = TokenResponse {
            access_token: "new".into(),
            refresh_token: Some("new-rt".into()),
            expires_in: Some(3600),
        };
        let patched = patch(&raw, "old", "old-rt", &token, "new-expiry").unwrap();
        let value: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(value["quota_remaining_tokens"], 42);
        assert_eq!(value["unknown_field"], "retained");
        assert_eq!(value["account_payload"]["refresh_token"], "new-rt");
        assert_eq!(
            value["account_payload"]["session_auth"]["expires_at"],
            "new-expiry"
        );
        assert!(patch(&patched, "old", "old-rt", &token, "new-expiry").is_none());
        assert!(patch(&raw, "old", "other-rt", &token, "new-expiry").is_none());
    }
}
