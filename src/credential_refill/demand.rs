use crate::credential_pool_automation::{
    archived_provider_credential_count, provider_credential_archive_path,
    provider_credential_storage_path,
};
use crate::error::GatewayError;
use crate::routing::config::ProviderConfigYaml;
use crate::state::AppState;

use super::*;

pub async fn list_credential_refill_demands(
    state: &AppState,
) -> Result<Vec<CredentialRefillDemandView>, GatewayError> {
    let snapshot = state.route_config.snapshot();
    let mut demands = Vec::with_capacity(snapshot.document().providers.len());
    for provider in &snapshot.document().providers {
        let outstanding = load_outstanding_task(state, &provider.id).await?;
        demands.push(build_demand_view(
            state,
            provider,
            snapshot.revision().id(),
            outstanding.as_ref(),
        ));
    }
    Ok(demands)
}

fn build_demand_view(
    state: &AppState,
    provider: &ProviderConfigYaml,
    revision_id: &str,
    outstanding: Option<&CredentialRefillTaskRecord>,
) -> CredentialRefillDemandView {
    let target_size = provider
        .pool_target_size
        .unwrap_or(1)
        .clamp(1, MAX_REQUESTED_COUNT);
    let active_credential_count = active_credential_count(provider);
    let category_deficit = identity_category_deficit(provider);
    let deficit = target_size
        .saturating_sub(active_credential_count)
        .max(category_deficit);
    let direct_driver_configured = state
        .credential_pool_automation
        .has_driver_for_provider(provider);
    let queue_enabled = state.credential_pool_automation.refill_queue_enabled();
    CredentialRefillDemandView {
        provider_id: provider.id.clone(),
        provider_label: provider
            .label
            .clone()
            .unwrap_or_else(|| provider.id.clone()),
        target_size,
        credential_count: provider.credentials.len(),
        active_credential_count,
        deficit,
        needs_refill: deficit > 0,
        auto_refill_enabled: provider.auto_refill_enabled,
        direct_driver_configured,
        notification_enabled: queue_enabled
            && provider.auto_refill_enabled
            && !direct_driver_configured,
        inquiry_enabled: queue_enabled,
        user_request_enabled: queue_enabled,
        outstanding_task_id: outstanding.map(|task| task.id.clone()),
        outstanding_task_state: outstanding.map(|task| task.state),
        notification_api: format!(
            "{CREDENTIAL_REFILL_ROOT_API}/providers/{}/tasks/claim",
            provider_path_segment(&provider.id)
        ),
        inquiry_api: format!(
            "{CREDENTIAL_REFILL_ROOT_API}/providers/{}",
            provider_path_segment(&provider.id)
        ),
        credential_storage_path: provider_credential_storage_path(&state.config, &provider.id)
            .map(|path| path.to_string_lossy().into_owned()),
        storage_password_configured: provider
            .credential_storage_password
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        archive_storage_path: provider_credential_archive_path(&state.config, &provider.id)
            .map(|path| path.to_string_lossy().into_owned()),
        archived_credential_count: archived_provider_credential_count(&state.config, &provider.id),
        permanent_delete_enabled: provider.credential_permanent_delete_enabled,
        revision_id: revision_id.to_string(),
    }
}

pub async fn demand_for_provider(
    state: &AppState,
    provider_id: &str,
) -> Result<CredentialRefillDemandView, GatewayError> {
    let provider_id = normalize_identifier(provider_id, "providerId", MAX_WORKER_ID_LENGTH)?;
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| {
            GatewayError::not_found(format!("Provider '{provider_id}' 不存在"))
                .with_code("credential_refill_provider_not_found")
        })?;
    let outstanding = load_outstanding_task(state, &provider_id).await?;
    Ok(build_demand_view(
        state,
        provider,
        snapshot.revision().id(),
        outstanding.as_ref(),
    ))
}

pub(super) fn provider_path_segment(provider_id: &str) -> String {
    url::form_urlencoded::byte_serialize(provider_id.as_bytes()).collect()
}

pub(super) fn active_credential_count(provider: &ProviderConfigYaml) -> usize {
    if provider.credentials.is_empty() {
        usize::from(!provider.api_key.trim().is_empty() || provider.auth_token.is_some())
    } else {
        provider
            .credentials
            .iter()
            .filter(|credential| credential.enabled.unwrap_or(true))
            .count()
    }
}

pub(super) fn identity_category_deficit(provider: &ProviderConfigYaml) -> usize {
    provider
        .credential_identity_categories
        .iter()
        .filter_map(serde_json::Value::as_object)
        .filter(|category| {
            category
                .get("auto_refill_enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|category| {
            let category_id = category.get("id")?.as_str()?.trim();
            if category_id.is_empty() {
                return None;
            }
            let target = category
                .get("pool_target_size")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(1)
                .clamp(1, MAX_REQUESTED_COUNT);
            let active = provider
                .credentials
                .iter()
                .filter(|credential| credential.enabled.unwrap_or(true))
                .filter(|credential| {
                    credential.credential_identity_category_id.as_deref() == Some(category_id)
                })
                .count();
            Some(target.saturating_sub(active))
        })
        .fold(0usize, usize::saturating_add)
        .min(MAX_REQUESTED_COUNT)
}
