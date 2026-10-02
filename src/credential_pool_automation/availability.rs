//! Match the console health rollup: an active model and no credential-wide failure.
use crate::{error::GatewayError, routing::config::ProviderConfigYaml, state::AppState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PoolAvailability {
    pub available: usize,
    pub reserved_unknown: usize,
    pub remaining: usize,
}

pub(crate) async fn pool_availability(
    state: &AppState,
    provider: &ProviderConfigYaml,
) -> Result<PoolAvailability, GatewayError> {
    let refs = enabled_credential_refs(provider);
    let enabled = refs.len();
    let (available, observed): (i64, i64) = if enabled == 0 {
        (0, 0)
    } else if let Some(local) = &state.local_runtime {
        local_counts(&local.pool, &provider.id, &refs).await?
    } else if let Some(pool) = &state.pg_pool {
        sqlx::query_as(r#"
SELECT COUNT(*) FILTER (WHERE has_active AND NOT has_failure), COUNT(*) FROM (
 SELECT COALESCE(NULLIF(provider_credential_ref, ''), provider_credential_id) AS ref,
 BOOL_OR(lower(trim(status)) = 'active') AS has_active,
 BOOL_OR(lower(trim(status)) <> 'active' AND failure_scope IS DISTINCT FROM 'credential_model') AS has_failure
 FROM gateway_provider_credential_model_states
 WHERE provider_account_id = $1
 AND COALESCE(NULLIF(provider_credential_ref, ''), provider_credential_id) = ANY($2)
 GROUP BY ref
) AS health"#).bind(&provider.id).bind(refs).fetch_one(pool).await
            .map_err(|_| GatewayError::service_unavailable("Credential health is unavailable"))?
    } else {
        (0, 0)
    };
    let available = usize::try_from(available).unwrap_or(0);
    let reserved_unknown = enabled.saturating_sub(usize::try_from(observed).unwrap_or(0));
    Ok(PoolAvailability {
        available,
        reserved_unknown,
        remaining: super::capacity::pool_max_size(provider)
            .saturating_sub(available.saturating_add(reserved_unknown)),
    })
}

async fn local_counts(
    pool: &sqlx::SqlitePool,
    provider_id: &str,
    refs: &[String],
) -> Result<(i64, i64), GatewayError> {
    let refs = serde_json::to_string(refs)
        .map_err(|_| GatewayError::server_error("Cannot encode credential refs"))?;
    sqlx::query_as(r#"
SELECT COALESCE(SUM(has_active AND NOT has_failure), 0), COUNT(*) FROM (
 SELECT COALESCE(NULLIF(json_extract(payload, '$.providerCredentialRef'), ''), json_extract(payload, '$.providerCredentialId')) AS ref,
 MAX(lower(trim(json_extract(payload, '$.status'))) = 'active') AS has_active,
 MAX(lower(trim(json_extract(payload, '$.status'))) != 'active' AND COALESCE(json_extract(payload, '$.failureScope'), '') != 'credential_model') AS has_failure
 FROM credential_model_states
 WHERE json_extract(payload, '$.providerAccountId') = ?
 AND COALESCE(NULLIF(json_extract(payload, '$.providerCredentialRef'), ''), json_extract(payload, '$.providerCredentialId')) IN (SELECT value FROM json_each(?))
 GROUP BY ref
)"#).bind(provider_id).bind(refs).fetch_one(pool).await
            .map_err(crate::local_runtime::storage_error)
}

fn enabled_credential_refs(provider: &ProviderConfigYaml) -> Vec<String> {
    if provider.credentials.is_empty() {
        if super::capacity::active_credential_count(provider) == 0 {
            return Vec::new();
        }
        return vec![crate::routing::config::provider_default_account_id(
            &provider.id,
        )];
    }
    provider
        .credentials
        .iter()
        .enumerate()
        .filter(|(_, credential)| credential.enabled.unwrap_or(true))
        .map(|(index, credential)| {
            super::inventory::effective_credential_id(&provider.id, index, credential)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn health_matches_active_model_with_no_credential_wide_failure() {
        let root = std::env::temp_dir().join(format!(
            "gateway-local-pool-health-{}",
            uuid::Uuid::new_v4()
        ));
        let local = crate::local_runtime::LocalRuntime::open(&root)
            .await
            .unwrap();
        // Exercise the same SQL predicate without constructing unrelated AppState dependencies.
        let rows = [
            ("active", "a", "active", None),
            ("a-model", "a", "blocked", Some("credential_model")),
            ("b-active", "b", "active", None),
            ("b-auth", "b", "blocked", Some("credential")),
            ("cooling", "c", "cooling", Some("credential_model")),
        ];
        for (id, reference, status, scope) in rows {
            let payload = serde_json::json!({"providerAccountId":"p", "providerCredentialRef":reference, "status":status, "failureScope":scope});
            sqlx::query(
                "INSERT INTO credential_model_states(id, payload, updated) VALUES (?, ?, 0)",
            )
            .bind(id)
            .bind(payload.to_string())
            .execute(&local.pool)
            .await
            .unwrap();
        }
        let refs = ["a", "b", "c", "unobserved"].map(str::to_string);
        let (available, observed) = local_counts(&local.pool, "p", &refs).await.unwrap();
        assert_eq!((available, observed), (1, 3));
        // The cooling and credential-wide invalid rows do not occupy the target;
        // unobserved material reserves one slot while verification is pending.
        assert_eq!(refs.len() - observed as usize, 1);
        sqlx::query("DELETE FROM credential_model_states WHERE id = 'b-auth'")
            .execute(&local.pool)
            .await
            .unwrap();
        assert_eq!(local_counts(&local.pool, "p", &refs).await.unwrap(), (2, 3));
        local.close().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }
}
