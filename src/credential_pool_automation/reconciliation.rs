use super::archive::archive_pruned_credentials;
use super::driver::execute_driver;
use super::inventory::{
    active_credential_count, effective_credential_id, identity_category_requested_count,
    normalized_target_size, prune_credentials,
};
use super::registry::normalize_required_id;
use super::{
    CredentialAutomationDriver, CredentialPoolAutomationAction, DriverCredentialContext,
    DriverProviderContext, DriverRequest, ReconcileOutcome,
};
use crate::routing::config::ProviderConfigYaml;
use crate::state::AppState;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;
pub(super) async fn reconcile_provider_with_driver(
    state: &Arc<AppState>,
    snapshot: &crate::routing::config::RouteConfigSnapshot,
    provider: ProviderConfigYaml,
    driver: &CredentialAutomationDriver,
    action: CredentialPoolAutomationAction,
) -> anyhow::Result<ReconcileOutcome> {
    let target_size = normalized_target_size(provider.pool_target_size);
    let active_count = active_credential_count(&provider);
    let refill_enabled =
        action == CredentialPoolAutomationAction::Reconcile && provider.auto_refill_enabled;
    let prune_enabled =
        action == CredentialPoolAutomationAction::Prune || provider.auto_prune_enabled;
    let requested_count = if refill_enabled {
        target_size
            .saturating_sub(active_count)
            .max(identity_category_requested_count(&provider))
    } else {
        0
    };
    let request = DriverRequest {
        run_id: Uuid::new_v4().to_string(),
        action: action.as_str(),
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
            auto_refill_enabled: refill_enabled,
            auto_prune_enabled: prune_enabled,
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
        refill: None,
    };
    let response =
        execute_driver(&state.credential_pool_automation.config, driver, &request).await?;
    if action == CredentialPoolAutomationAction::Prune && !response.credentials.is_empty() {
        anyhow::bail!("prune driver action must not return credentials");
    }
    let mut document = snapshot.document().clone();
    let provider_index = document
        .providers
        .iter()
        .position(|candidate| candidate.id == provider.id)
        .ok_or_else(|| anyhow::anyhow!("provider disappeared from route document"))?;
    let target_provider = &mut document.providers[provider_index];
    let mut existing_ids = target_provider
        .credentials
        .iter()
        .enumerate()
        .map(|(index, credential)| effective_credential_id(&target_provider.id, index, credential))
        .collect::<HashSet<_>>();

    let mut created_count = 0usize;
    if refill_enabled {
        for credential in response.credentials.into_iter().take(requested_count) {
            let credential_id = credential
                .id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("driver returned a credential without an id"))?
                .to_string();
            normalize_required_id(&credential_id, "credential id")?;
            if !existing_ids.insert(credential_id) {
                anyhow::bail!("driver returned a duplicate credential id");
            }
            target_provider.credentials.push(credential);
            created_count += 1;
        }
    }

    let mut prune_ids = HashSet::new();
    if prune_enabled {
        for decision in response.prune {
            let _classification = decision.classification;
            if existing_ids.contains(&decision.credential_id) {
                prune_ids.insert(decision.credential_id);
            }
        }
    }
    let archived_count = archive_pruned_credentials(&state.config, target_provider, &prune_ids)?;
    let pruned_count = prune_credentials(target_provider, &prune_ids);
    if !target_provider.credential_permanent_delete_enabled && archived_count != pruned_count {
        anyhow::bail!("credential archive count did not match the prune set");
    }

    if created_count == 0 && pruned_count == 0 {
        return Ok(ReconcileOutcome {
            created_count,
            pruned_count,
            message: response.message.or_else(|| {
                Some(match action {
                    CredentialPoolAutomationAction::Reconcile => {
                        "凭证池已检查，无需修改。".to_string()
                    }
                    CredentialPoolAutomationAction::Prune => "失效号已检查，无需删除。".to_string(),
                })
            }),
            revision_id: Some(snapshot.revision().id().to_string()),
        });
    }
    let runtime = state
        .route_config_runtime
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("route configuration runtime is unavailable"))?;
    let committed = runtime
        .commit_automation_document(
            snapshot.revision().id(),
            document,
            Some(format!(
                "credential pool {} for {}: +{}, -{}",
                action.as_str(),
                provider.id,
                created_count,
                pruned_count
            )),
        )
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(ReconcileOutcome {
        created_count,
        pruned_count,
        message: response.message.or_else(|| {
            Some(match action {
                CredentialPoolAutomationAction::Reconcile => {
                    format!("自动补号 {created_count} 个，自动删除失效号 {pruned_count} 个。")
                }
                CredentialPoolAutomationAction::Prune => {
                    format!("手动删除失效号 {pruned_count} 个。")
                }
            })
        }),
        revision_id: Some(committed.revision().id().to_string()),
    })
}
