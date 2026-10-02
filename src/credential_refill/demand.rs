use crate::credential_pool_automation::capacity::{pool_max_size, pool_min_size};
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
        demands.push(
            build_demand_view(
                state,
                provider,
                snapshot.revision().id(),
                outstanding.as_ref(),
            )
            .await?,
        );
    }
    Ok(demands)
}

async fn build_demand_view(
    state: &AppState,
    provider: &ProviderConfigYaml,
    revision_id: &str,
    outstanding: Option<&CredentialRefillTaskRecord>,
) -> Result<CredentialRefillDemandView, GatewayError> {
    let target_size = pool_max_size(provider);
    let active_credential_count = active_credential_count(provider);
    let availability =
        crate::credential_pool_automation::availability::pool_availability(state, provider).await?;
    let deficit = availability.remaining;
    let available_credential_count = availability.available;
    let automatic_count = state
        .credential_pool_automation
        .automatic_refill_count(provider, availability);
    let direct_driver_configured = state
        .credential_pool_automation
        .has_driver_for_provider(provider);
    let queue_enabled = state.credential_pool_automation.refill_queue_enabled();
    Ok(CredentialRefillDemandView {
        provider_id: provider.id.clone(),
        provider_label: provider
            .label
            .clone()
            .unwrap_or_else(|| provider.id.clone()),
        target_size,
        min_size: pool_min_size(provider),
        available_credential_count,
        credential_count: provider.credentials.len(),
        active_credential_count,
        deficit,
        needs_refill: automatic_count > 0,
        auto_refill_enabled: provider.auto_refill_enabled,
        direct_driver_configured,
        notification_enabled: queue_enabled
            && provider.auto_refill_enabled
            && !direct_driver_configured,
        inquiry_enabled: queue_enabled && provider.auto_refill_enabled && !direct_driver_configured,
        user_request_enabled: queue_enabled && deficit > 0,
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
        credential_storage_path: provider_credential_storage_path(&state.config, provider)
            .map(|path| path.to_string_lossy().into_owned()),
        storage_password_configured: provider
            .credential_storage_password
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        archive_storage_path: provider_credential_archive_path(&state.config, provider)
            .map(|path| path.to_string_lossy().into_owned()),
        archived_credential_count: archived_provider_credential_count(&state.config, provider),
        permanent_delete_enabled: provider.credential_permanent_delete_enabled,
        revision_id: revision_id.to_string(),
    })
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
    build_demand_view(
        state,
        provider,
        snapshot.revision().id(),
        outstanding.as_ref(),
    )
    .await
}

pub(super) fn provider_path_segment(provider_id: &str) -> String {
    url::form_urlencoded::byte_serialize(provider_id.as_bytes()).collect()
}

pub(super) use crate::credential_pool_automation::capacity::active_credential_count;
