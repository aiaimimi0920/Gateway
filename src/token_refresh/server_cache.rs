//! Server cache updates compare the original material and CAS the complete raw entry.
use super::transport::TokenResponse;
use redis::AsyncCommands;
use serde_json::{json, Value};

pub(super) const CAS: &str = r#"
if redis.call('GET', KEYS[1]) ~= ARGV[1] then return 0 end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
return 1
"#;

pub(crate) fn patch_from_grant(
    raw: &str,
    record: &crate::console::refresh_grants::GrantRecord,
    target: &crate::routing::candidate::ProviderAccountPayload,
) -> Result<Option<(String, i64)>, time::error::Parse> {
    if !super::cache_target::matches(raw, record, target) {
        return Ok(None);
    }
    let Some(result) = record.result() else {
        return Ok(None);
    };
    let original = record.original();
    let Some(expiry) = result.expires_at.as_deref() else {
        return Ok(None);
    };
    let token = TokenResponse {
        access_token: result.material.access.clone(),
        refresh_token: Some(result.material.refresh.clone()),
        expires_in: result.expires_in,
        grok_oauth: result.material.context.clone(),
        vertex_oauth: result.material.vertex_oauth.clone(),
        dim_oauth: result.material.dim_oauth.clone(),
        nous_portal: result.material.nous_portal.clone(),
        lobsterai: result.material.lobsterai.clone(),
        absolute_expiry: result.expires_at.clone(),
        accio_sgcookie: result.material.accio_sgcookie.clone(),
    };
    let Some(updated) = patch_with_nous(
        raw,
        &original.access,
        &original.refresh,
        original.context.as_ref(),
        original.kiro_oauth.as_ref(),
        original.vertex_oauth.as_ref(),
        original.dim_oauth.as_ref(),
        original.nous_portal.as_ref(),
        original.accio_sgcookie.as_deref(),
        &token,
        expiry,
    ) else {
        return Ok(None);
    };
    Ok(Some((updated, cache_ttl(expiry)?)))
}

fn cache_ttl(expiry: &str) -> Result<i64, time::error::Parse> {
    Ok(
        time::OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)?
            .unix_timestamp()
            .saturating_sub(time::OffsetDateTime::now_utc().unix_timestamp())
            .max(1),
    )
}

#[cfg(test)]
fn patch(
    raw: &str,
    access: &str,
    refresh: &str,
    context: Option<&crate::protocol::grok_oauth::AuthorizationContext>,
    kiro_context: Option<&crate::protocol::kiro_oauth::AuthorizationContext>,
    vertex_context: Option<&crate::protocol::vertex_oauth::AuthorizationContext>,
    dim_context: Option<&crate::protocol::dim_oauth::AuthorizationContext>,
    sgcookie: Option<&str>,
    token: &TokenResponse,
    expiry: &str,
) -> Option<String> {
    patch_with_nous(
        raw,
        access,
        refresh,
        context,
        kiro_context,
        vertex_context,
        dim_context,
        None,
        sgcookie,
        token,
        expiry,
    )
}

fn patch_with_nous(
    raw: &str,
    access: &str,
    refresh: &str,
    context: Option<&crate::protocol::grok_oauth::AuthorizationContext>,
    kiro_context: Option<&crate::protocol::kiro_oauth::AuthorizationContext>,
    vertex_context: Option<&crate::protocol::vertex_oauth::AuthorizationContext>,
    dim_context: Option<&crate::protocol::dim_oauth::AuthorizationContext>,
    nous_context: Option<&crate::protocol::nous_portal::AuthorizationContext>,
    sgcookie: Option<&str>,
    token: &TokenResponse,
    expiry: &str,
) -> Option<String> {
    let mut entry: Value = serde_json::from_str(raw).ok()?;
    if !super::cache_target::matches_nous(entry.get("account_payload"), nous_context) {
        return None;
    }
    if let Some(context) = nous_context {
        let next = token.nous_portal.as_ref()?;
        if !context.permits_rotation(next)
            || !next.matches_expiry(expiry)
            || next
                .validate_binding(
                    &token.access_token,
                    &crate::protocol::nous_portal::OAuthClient::new(next.client_id()).ok()?,
                )
                .is_err()
            || entry
                .get("account_payload")
                .and_then(|p| p.get("base_url").or_else(|| p.get("baseUrl")))
                .is_some_and(|value| {
                    value.as_str().is_none_or(|s| {
                        s.trim_end_matches('/') != context.inference_target().base_url()
                    })
                })
        {
            return None;
        }
        if entry
            .get("api_base_url")
            .and_then(Value::as_str)?
            .trim_end_matches('/')
            != context.inference_target().base_url()
            || !entry
                .get("expires_at")
                .and_then(Value::as_str)
                .is_some_and(|value| context.matches_expiry(value))
            || entry
                .pointer("/account_payload/expires_at")
                .is_some_and(|value| {
                    !value
                        .as_str()
                        .is_some_and(|value| context.matches_expiry(value))
                })
        {
            return None;
        }
    } else if token.nous_portal.is_some() {
        return None;
    }
    if let Some(target) = kiro_context.and_then(|context| context.runtime_target()) {
        let parsed: crate::redis::credential_cache::CredentialEntry =
            serde_json::from_str(raw).ok()?;
        if parsed
            .api_base_url
            .as_deref()
            .map(|base| base.trim_end_matches('/'))
            != Some(target.base_url())
        {
            return None;
        }
    }
    if !super::cache_target::matches_kiro(entry.get("account_payload"), kiro_context) {
        return None;
    }
    if !super::cache_target::matches_vertex(entry.get("account_payload"), vertex_context) {
        return None;
    }
    if !super::cache_target::matches_dim(entry.get("account_payload"), dim_context) {
        return None;
    }
    if let Some(context) = dim_context {
        let saved_base = entry.get("api_base_url").and_then(Value::as_str)?;
        if saved_base.trim_end_matches('/')
            != crate::protocol::dim_oauth::business::base(context.site())
        {
            return None;
        }
    }
    if !super::cache_target::matches_sgcookie(entry.get("account_payload"), sgcookie) {
        return None;
    }
    let saved_context = entry
        .pointer("/account_payload/grok_oauth")
        .cloned()
        .unwrap_or(Value::Null);
    if saved_context != serde_json::to_value(context).ok()? {
        return None;
    }
    if entry.get("api_key")?.as_str()? != access {
        return None;
    }
    let rt = entry
        .pointer("/account_payload/refresh_token")
        .and_then(Value::as_str);
    if (kiro_context.is_some()
        || vertex_context.is_some()
        || dim_context.is_some()
        || nous_context.is_some())
        && rt != Some(refresh)
        || rt.is_some_and(|rt| rt != refresh)
    {
        return None;
    }
    entry["api_key"] = json!(token.access_token);
    entry["expires_at"] = json!(expiry);
    entry["updated_at"] = json!(crate::routing::config::future_rfc3339_after_secs(0));
    if let Some(context) = &token.nous_portal {
        entry["api_base_url"] = json!(context.inference_target().base_url());
    }
    if !entry.get("account_payload").is_some_and(Value::is_object) {
        entry["account_payload"] = json!({});
    }
    let payload = &mut entry["account_payload"];
    if let Some(cookie) = &token.accio_sgcookie {
        payload.as_object_mut()?.remove("accioSgcookie");
        payload["accio_sgcookie"] = json!(cookie);
    }
    if let Some(context) = &token.grok_oauth {
        payload["grok_oauth"] = serde_json::to_value(context).ok()?;
    }
    if let Some(context) = &token.vertex_oauth {
        payload["vertex_oauth"] = serde_json::to_value(context).ok()?;
    }
    if let Some(context) = &token.dim_oauth {
        payload["dim_oauth"] = serde_json::to_value(context).ok()?;
    }
    if let Some(context) = &token.nous_portal {
        payload["nous_portal"] = serde_json::to_value(context).ok()?;
        payload.as_object_mut()?.remove("baseUrl");
        payload["base_url"] = json!(context.inference_target().base_url());
    }
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

#[cfg(test)]
pub(super) async fn persist(
    pool: &deadpool_redis::Pool,
    id: &str,
    expected_access: &str,
    expected_refresh: &str,
    context: Option<&crate::protocol::grok_oauth::AuthorizationContext>,
    kiro_context: Option<&crate::protocol::kiro_oauth::AuthorizationContext>,
    vertex_context: Option<&crate::protocol::vertex_oauth::AuthorizationContext>,
    dim_context: Option<&crate::protocol::dim_oauth::AuthorizationContext>,
    sgcookie: Option<&str>,
    token: &TokenResponse,
    expiry: &str,
) -> anyhow::Result<()> {
    persist_with_nous(
        pool,
        id,
        expected_access,
        expected_refresh,
        context,
        kiro_context,
        vertex_context,
        dim_context,
        None,
        sgcookie,
        token,
        expiry,
    )
    .await
}

pub(super) async fn persist_with_nous(
    pool: &deadpool_redis::Pool,
    id: &str,
    expected_access: &str,
    expected_refresh: &str,
    context: Option<&crate::protocol::grok_oauth::AuthorizationContext>,
    kiro_context: Option<&crate::protocol::kiro_oauth::AuthorizationContext>,
    vertex_context: Option<&crate::protocol::vertex_oauth::AuthorizationContext>,
    dim_context: Option<&crate::protocol::dim_oauth::AuthorizationContext>,
    nous_context: Option<&crate::protocol::nous_portal::AuthorizationContext>,
    sgcookie: Option<&str>,
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
            let Some(updated) = patch_with_nous(
                &raw,
                expected_access,
                expected_refresh,
                context,
                kiro_context,
                vertex_context,
                dim_context,
                nous_context,
                sgcookie,
                token,
                expiry,
            ) else {
                return Ok(()); // Removed or reauthorized: never restore retired material.
            };
            let ttl = cache_ttl(expiry)?;
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
            grok_oauth: None,
            absolute_expiry: None,
            accio_sgcookie: None,
            vertex_oauth: None,
            dim_oauth: None,
            nous_portal: None,
            lobsterai: None,
            access_token: "new".into(),
            refresh_token: Some("new-rt".into()),
            expires_in: Some(3600),
        };
        let patched = patch(
            &raw,
            "old",
            "old-rt",
            None,
            None,
            None,
            None,
            None,
            &token,
            "new-expiry",
        )
        .unwrap();
        let value: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(value["quota_remaining_tokens"], 42);
        assert_eq!(value["unknown_field"], "retained");
        assert_eq!(value["account_payload"]["refresh_token"], "new-rt");
        assert_eq!(
            value["account_payload"]["session_auth"]["expires_at"],
            "new-expiry"
        );
        assert!(patch(
            &patched,
            "old",
            "old-rt",
            None,
            None,
            None,
            None,
            None,
            &token,
            "new-expiry"
        )
        .is_none());
        assert!(patch(
            &raw,
            "old",
            "other-rt",
            None,
            None,
            None,
            None,
            None,
            &token,
            "new-expiry"
        )
        .is_none());
    }

    #[test]
    fn grok_cache_patch_cas_binds_original_context_and_persists_scope_narrowing() {
        let value = json!({"issuer":"https://auth.x.ai","client_id":"explicit-client",
            "requested_scopes":["openid","profile"],"granted_scopes":["openid","profile"]});
        let context: crate::protocol::grok_oauth::AuthorizationContext =
            serde_json::from_value(value.clone()).unwrap();
        let mut narrowed = value;
        narrowed["granted_scopes"] = json!(["profile"]);
        let next: crate::protocol::grok_oauth::AuthorizationContext =
            serde_json::from_value(narrowed).unwrap();
        let raw = json!({"api_key":"old","account_payload":{"refresh_token":"old-rt",
            "grok_oauth":context},"quota_remaining_tokens":42})
        .to_string();
        let token = TokenResponse {
            access_token: "new".into(),
            refresh_token: Some("new-rt".into()),
            expires_in: Some(3600),
            grok_oauth: Some(next.clone()),
            absolute_expiry: None,
            accio_sgcookie: None,
            vertex_oauth: None,
            dim_oauth: None,
            nous_portal: None,
            lobsterai: None,
        };
        let patched = patch(
            &raw,
            "old",
            "old-rt",
            Some(&context),
            None,
            None,
            None,
            None,
            &token,
            "new-expiry",
        )
        .unwrap();
        let stored: Value = serde_json::from_str(&patched).unwrap();
        assert_eq!(
            stored["account_payload"]["grok_oauth"],
            serde_json::to_value(&next).unwrap()
        );
        assert_eq!(stored["quota_remaining_tokens"], 42);
        assert!(patch(
            &raw,
            "old",
            "old-rt",
            None,
            None,
            None,
            None,
            None,
            &token,
            "new-expiry"
        )
        .is_none());
        assert!(patch(
            &raw,
            "old",
            "old-rt",
            Some(&next),
            None,
            None,
            None,
            None,
            &token,
            "new-expiry"
        )
        .is_none());
        assert!(patch(
            &patched,
            "new",
            "new-rt",
            Some(&context),
            None,
            None,
            None,
            None,
            &token,
            "later-expiry"
        )
        .is_none());
        assert!(patch(
            &patched,
            "new",
            "new-rt",
            Some(&next),
            None,
            None,
            None,
            None,
            &token,
            "later-expiry"
        )
        .is_some());
    }
}
