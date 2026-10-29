//! A real provider toggle is an explicit health reset; ordinary edits and startup are not.
use super::*;
use crate::{redis::keys, routing::config::RouteConfigYaml};

pub(crate) async fn apply_provider_toggles(
    state: &AppState,
    before: &RouteConfigYaml,
    after: &RouteConfigYaml,
) -> Result<(), GatewayError> {
    let changed: Vec<_> = after
        .providers
        .iter()
        .filter(|provider| {
            before
                .providers
                .iter()
                .any(|old| old.id == provider.id && resets_health(old, provider))
        })
        .map(|p| p.id.clone())
        .collect();
    if changed.is_empty() {
        return Ok(());
    }
    let mut journal = state.lifecycle.health_control.gate.lock().await;
    state
        .lifecycle
        .health_control
        .reset_at
        .lock()
        .retain(|id, _| after.providers.iter().any(|provider| provider.id == *id));
    for provider in changed {
        {
            let mut resets = state.lifecycle.health_control.reset_at.lock();
            if resets.len() >= 8192 && !resets.contains_key(&provider) {
                return Err(GatewayError::service_unavailable(
                    "Provider reset journal is full",
                ));
            }
            resets.insert(provider.clone(), Instant::now());
        }
        // Invalidate leases before I/O, so old completions cannot re-trip the cleared entry.
        state.lifecycle.model_circuits.reset_provider(&provider);
        if let Some(local) = &state.local_runtime {
            local.reset_provider_health(&provider).await?;
        } else if let Some(pool) = &state.pg_pool {
            control::reset(pool, &provider).await?;
            clear_breaker_keys(state, &provider).await?;
        }
        journal
            .rows
            .retain(|_, (_, row)| row.row.provider_account_id != provider);
        state.lifecycle.smart_routing.reset_provider(&provider);
    }
    Ok(())
}

pub(super) fn dispatch_enabled(provider: &crate::routing::config::ProviderConfigYaml) -> bool {
    provider.uses_provider_default_account()
        || provider
            .credentials
            .iter()
            .any(|credential| credential.enabled.unwrap_or(true))
}

fn resets_health(
    before: &crate::routing::config::ProviderConfigYaml,
    after: &crate::routing::config::ProviderConfigYaml,
) -> bool {
    if dispatch_enabled(before) != dispatch_enabled(after) {
        return true;
    }
    if after.credentials.is_empty() || !after.credentials.iter().all(|c| c.enabled.unwrap_or(true))
    {
        return false;
    }
    // Index only paused identities: ordinary edits and large-pool resumes stay linear.
    let paused: std::collections::HashSet<_> = before
        .credentials
        .iter()
        .filter(|c| c.enabled == Some(false))
        .map(|c| &c.id)
        .collect();
    after.credentials.iter().any(|c| paused.contains(&c.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_pool_resume_resets_but_single_pause_and_metadata_edits_do_not() {
        let before: crate::routing::config::ProviderConfigYaml = serde_json::from_value(serde_json::json!({
            "id":"site","preset":"openai","base_url":"http://127.0.0.1:1/v1","credentials":[{"id":"a","enabled":false},{"id":"b","enabled":true}]
        })).unwrap();
        let mut after = before.clone();
        after.label = Some("new label".into());
        assert!(!resets_health(&before, &after));
        after.credentials[0].enabled = Some(true);
        assert!(resets_health(&before, &after));
        assert!(!resets_health(&after, &before));
        after.credentials[0].enabled = Some(false);
        after.credentials[1].enabled = Some(false);
        assert!(resets_health(&before, &after));
    }
}

async fn clear_breaker_keys(state: &AppState, provider: &str) -> Result<(), GatewayError> {
    let pool = state.pg_pool.as_ref().expect("server reset has PostgreSQL");
    let mut connection =
        state.redis_pool.get().await.map_err(|_| {
            GatewayError::service_unavailable("Cannot reset provider breaker storage")
        })?;
    // DEL only health keys. Quota, billing reservations, locks and actual credentials are untouched.
    let mut cursor = String::new();
    loop {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM gateway_provider_credentials
            WHERE provider_account_id=$1 AND id>$2 ORDER BY id LIMIT 256",
        )
        .bind(provider)
        .bind(&cursor)
        .fetch_all(pool)
        .await
        .map_err(db::map_db_error)?;
        if ids.is_empty() {
            break;
        }
        let mut command = redis::cmd("DEL");
        for id in &ids {
            command
                .arg(keys::provider_credential_failure_count_key(id))
                .arg(keys::provider_credential_breaker_open_key(id));
        }
        command
            .query_async::<usize>(&mut connection)
            .await
            .map_err(|_| {
                GatewayError::service_unavailable("Cannot reset credential breaker storage")
            })?;
        cursor = ids.last().cloned().unwrap_or_default();
    }
    redis::cmd("DEL")
        .arg(keys::provider_failure_count_key(provider))
        .arg(keys::provider_breaker_open_key(provider))
        .query_async::<usize>(&mut connection)
        .await
        .map_err(|_| GatewayError::service_unavailable("Cannot reset provider breaker storage"))?;
    Ok(())
}
