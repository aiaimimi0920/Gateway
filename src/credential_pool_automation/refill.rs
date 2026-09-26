use super::driver::execute_driver;
use super::inventory::{active_credential_count, effective_credential_id, normalized_target_size};
use super::{DriverCredentialContext, DriverProviderContext, DriverRefillContext, DriverRequest};
use crate::routing::config::ProviderCredentialYaml;
use crate::state::AppState;
use std::sync::Arc;
use uuid::Uuid;
pub async fn collect_refill_credentials_from_driver(
    state: &Arc<AppState>,
    provider_id: &str,
    task_id: &str,
    requested_count: usize,
    artifact_reference: &str,
) -> anyhow::Result<Vec<ProviderCredentialYaml>> {
    if !state.credential_pool_automation.enabled() {
        anyhow::bail!("credential pool automation is disabled");
    }
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("provider '{provider_id}' was not found"))?;
    let driver = state
        .credential_pool_automation
        .registry
        .resolve(&provider)
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow::anyhow!("该渠道尚未配置自动补号驱动器"))?
        .clone();
    let target_size = normalized_target_size(provider.pool_target_size);
    let active_count = active_credential_count(&provider);
    let request = DriverRequest {
        run_id: Uuid::new_v4().to_string(),
        action: "collect_refill",
        provider: DriverProviderContext {
            id: provider.id.clone(),
            label: provider
                .label
                .clone()
                .unwrap_or_else(|| provider.id.clone()),
            target_size,
            credential_count: provider.credentials.len(),
            active_credential_count: active_count,
            requested_count,
            auto_refill_enabled: provider.auto_refill_enabled,
            auto_prune_enabled: false,
            identity_categories: provider.credential_identity_categories.clone(),
            credentials: provider
                .credentials
                .iter()
                .enumerate()
                .map(|(index, credential)| DriverCredentialContext {
                    id: effective_credential_id(&provider.id, index, credential),
                    account_name: credential.account_name.clone(),
                    enabled: credential.enabled.unwrap_or(true),
                    identity_category_id: credential.credential_identity_category_id.clone(),
                })
                .collect(),
        },
        refill: Some(DriverRefillContext {
            task_id: task_id.to_string(),
            requested_count,
            artifact_reference: artifact_reference.to_string(),
        }),
    };
    let response =
        execute_driver(&state.credential_pool_automation.config, &driver, &request).await?;
    if !response.prune.is_empty() {
        anyhow::bail!("refill collection driver must not return prune decisions");
    }
    Ok(response.credentials)
}
