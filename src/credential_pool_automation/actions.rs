use super::reconciliation::reconcile_provider_with_driver;
use super::runtime::{next_run_at, now_rfc3339};
use super::{
    CredentialPoolAutomationAction, ProviderCredentialPoolAutomationView, ProviderRunState,
};
use crate::state::AppState;
use std::sync::Arc;
pub async fn start_credential_pool_automation_task(state: Arc<AppState>) {
    if !state.credential_pool_automation.enabled() {
        tracing::info!("credential pool automation worker is disabled");
        return;
    }
    loop {
        sweep_credential_pool_automation_once(&state).await;
        tokio::time::sleep(state.credential_pool_automation.interval()).await;
    }
}

pub async fn sweep_credential_pool_automation_once(state: &Arc<AppState>) {
    let provider_ids = state
        .route_config
        .snapshot()
        .document()
        .providers
        .iter()
        .filter(|provider| {
            (provider.auto_refill_enabled || provider.auto_prune_enabled)
                && state
                    .credential_pool_automation
                    .has_driver_for_provider(provider)
        })
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    for provider_id in provider_ids {
        if let Err(error) = run_provider_credential_pool_automation(state, &provider_id).await {
            tracing::warn!(provider_id, error = %error, "credential pool automation sweep failed");
        }
    }
}

pub async fn run_provider_credential_pool_automation(
    state: &Arc<AppState>,
    provider_id: &str,
) -> anyhow::Result<ProviderCredentialPoolAutomationView> {
    run_provider_credential_pool_action(
        state,
        provider_id,
        CredentialPoolAutomationAction::Reconcile,
    )
    .await
}

pub async fn run_provider_credential_pool_prune(
    state: &Arc<AppState>,
    provider_id: &str,
) -> anyhow::Result<ProviderCredentialPoolAutomationView> {
    run_provider_credential_pool_action(state, provider_id, CredentialPoolAutomationAction::Prune)
        .await
}

async fn run_provider_credential_pool_action(
    state: &Arc<AppState>,
    provider_id: &str,
    action: CredentialPoolAutomationAction,
) -> anyhow::Result<ProviderCredentialPoolAutomationView> {
    if !state.credential_pool_automation.enabled() {
        anyhow::bail!("credential pool automation is disabled");
    }
    let provider_lock = state
        .credential_pool_automation
        .lock_for_provider(provider_id);
    let _guard = provider_lock.lock().await;
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("provider '{provider_id}' was not found"))?;
    if action == CredentialPoolAutomationAction::Reconcile
        && !provider.auto_refill_enabled
        && !provider.auto_prune_enabled
    {
        anyhow::bail!("provider '{provider_id}' has not enabled automatic refill or prune");
    }
    let driver = state
        .credential_pool_automation
        .registry
        .resolve(&provider)
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow::anyhow!("该渠道尚未配置自动补号驱动器"))?
        .clone();
    let started_at = now_rfc3339();
    state
        .credential_pool_automation
        .set_state(
            provider_id,
            ProviderRunState {
                state: "running".to_string(),
                last_run_at: Some(started_at.clone()),
                last_action: Some(action.as_str().to_string()),
                ..ProviderRunState::default()
            },
        )
        .await;

    let result = reconcile_provider_with_driver(state, &snapshot, provider, &driver, action).await;
    let final_state = match &result {
        Ok(outcome) => ProviderRunState {
            state: "succeeded".to_string(),
            last_run_at: Some(started_at),
            next_run_at: Some(next_run_at(state.credential_pool_automation.interval())),
            last_action: Some(action.as_str().to_string()),
            created_count: outcome.created_count,
            pruned_count: outcome.pruned_count,
            message: outcome.message.clone(),
            revision_id: outcome.revision_id.clone(),
        },
        Err(error) => ProviderRunState {
            state: "failed".to_string(),
            last_run_at: Some(started_at),
            next_run_at: Some(next_run_at(state.credential_pool_automation.interval())),
            last_action: Some(action.as_str().to_string()),
            message: Some(sanitize_driver_error(error)),
            ..ProviderRunState::default()
        },
    };
    state
        .credential_pool_automation
        .set_state(provider_id, final_state)
        .await;
    result?;

    state
        .credential_pool_automation
        .provider_views(&state.route_config.snapshot().document().providers)
        .await
        .into_iter()
        .find(|view| view.provider_id == provider_id)
        .ok_or_else(|| anyhow::anyhow!("provider automation status disappeared"))
}

fn sanitize_driver_error(error: &anyhow::Error) -> String {
    let message = error.to_string();
    if message.chars().count() <= 320 {
        message
    } else {
        format!("{}...", message.chars().take(320).collect::<String>())
    }
}
