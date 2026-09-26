//! Credential inventory and quota fallback for route-document storage.

use super::super::provider_credential_quota_batch::collect_bounded_ordered;
use super::credential_payload::mask_provider_payload_secrets;
use crate::error::GatewayError;
use crate::provider_quota;
use crate::state::AppState;
use serde_json::Value;
use std::collections::HashMap;

pub(super) async fn list_route_config_provider_credentials(
    state: &AppState,
    mask_secrets: bool,
) -> Result<Value, GatewayError> {
    let snapshot = state.route_config.snapshot();
    let redis_pool = &state.redis_pool;
    let mut jobs = Vec::new();

    for provider in &snapshot.document().providers {
        let Some(targets) = snapshot.credential_probe_targets_for_provider(&provider.id) else {
            continue;
        };
        let display_labels = provider
            .credentials
            .iter()
            .filter_map(|credential| {
                let id = credential.id.as_deref()?.trim();
                (!id.is_empty()).then(|| (id.to_owned(), credential.account_name.clone()))
            })
            .collect::<HashMap<_, _>>();
        for target in targets {
            let display_label = display_labels
                .get(target.credential_id.as_str())
                .cloned()
                .flatten()
                .or_else(|| provider.account_name.clone())
                .unwrap_or_else(|| target.credential_id.clone());
            jobs.push(async move {
                let credential_payload =
                    serde_json::to_value(&target.payload).map_err(|error| {
                        GatewayError::server_error(format!(
                            "serialize route credential {}: {error}",
                            target.credential_id
                        ))
                    })?;
                let quota = provider_quota::read_cached_runtime_quota_snapshot(
                    redis_pool,
                    target.provider_id.as_str(),
                    Some(target.credential_id.as_str()),
                )
                .await
                .ok()
                .flatten();
                Ok::<_, GatewayError>(serde_json::json!({
                    "id": target.credential_id,
                    "providerAccountId": target.provider_id,
                    "label": display_label,
                    "status": if target.enabled { "active" } else { "disabled" },
                    "credential": if mask_secrets {
                        mask_provider_payload_secrets(credential_payload)
                    } else {
                        credential_payload
                    },
                    "sourceKind": "route_config",
                    "sourcePath": Value::Null,
                    "syncMode": "route_config",
                    "syncState": "available",
                    "providerQuota": quota,
                }))
            });
        }
    }
    let credentials = collect_bounded_ordered(jobs)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(serde_json::json!({ "credentials": credentials }))
}

pub(super) async fn load_route_config_provider_credential_quota(
    state: &AppState,
    provider_credential_id: &str,
    force_refresh: bool,
) -> Result<Option<provider_quota::GatewayProviderQuotaView>, GatewayError> {
    let target = state
        .route_config
        .snapshot()
        .select_credential_probe_target(provider_credential_id)
        .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))?;
    if force_refresh {
        provider_quota::refresh_runtime_quota_snapshot(
            &state.redis_pool,
            state.config.upstream_timeout_secs,
            target.provider_id.as_str(),
            Some(target.credential_id.as_str()),
            &target.payload,
        )
        .await
    } else {
        Ok(provider_quota::read_cached_runtime_quota_snapshot(
            &state.redis_pool,
            target.provider_id.as_str(),
            Some(target.credential_id.as_str()),
        )
        .await
        .ok()
        .flatten())
    }
}
