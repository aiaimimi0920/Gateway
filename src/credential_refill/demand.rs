use crate::credential_pool_automation::capacity::{pool_max_size, pool_min_size};
use crate::credential_pool_automation::{
    provider_credential_archive_path, provider_credential_storage_path,
};
use crate::error::GatewayError;
use crate::routing::config::ProviderConfigYaml;
use crate::state::AppState;
use futures::{stream::FuturesUnordered, StreamExt};
use std::time::Duration;

use super::*;

pub async fn list_credential_refill_demands(
    state: &AppState,
) -> Result<Vec<CredentialRefillDemandView>, GatewayError> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let snapshot = state.route_config.snapshot();
        // Keep at most four concrete futures, without an async mapping closure over
        // borrowed providers (which breaks the complete Axum handler's Send bound).
        let mut providers = snapshot.document().providers.iter().enumerate();
        let mut tasks = FuturesUnordered::new();
        for (index, provider) in providers.by_ref().take(4) {
            tasks.push(indexed_demand_view(
                state,
                provider,
                snapshot.revision().id(),
                index,
            ));
        }
        let mut results = Vec::with_capacity(snapshot.document().providers.len());
        while let Some(result) = tasks.next().await {
            results.push(result?);
            if let Some((index, provider)) = providers.next() {
                tasks.push(indexed_demand_view(
                    state,
                    provider,
                    snapshot.revision().id(),
                    index,
                ));
            }
        }
        results.sort_by_key(|(index, _)| *index);
        Ok(results.into_iter().map(|(_, view)| view).collect())
    })
    .await
    .map_err(|_| {
        GatewayError::service_unavailable("Credential refill status deadline exceeded")
            .with_code("credential_refill_status_timeout")
    })?
}

async fn indexed_demand_view(
    state: &AppState,
    provider: &ProviderConfigYaml,
    revision_id: &str,
    index: usize,
) -> Result<(usize, CredentialRefillDemandView), GatewayError> {
    let outstanding = load_outstanding_task(state, &provider.id).await?;
    let view = build_demand_view(state, provider, revision_id, outstanding.as_ref()).await?;
    Ok((index, view))
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
    let archive_count = tokio::time::timeout(
        Duration::from_secs(5),
        crate::credential_pool_storage::archive::count(&state.config, provider),
    )
    .await
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Archive count timed out")));
    let (archive_purge_supported, archive_purge_unsupported_reason) =
        crate::credential_pool_storage::archive_purge_support(
            provider.credential_archive_connection.as_ref(),
        );
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
            .map(|path| path.to_string_lossy().into_owned())
            .or_else(|| remote_location(provider, false)),
        storage_password_configured: provider
            .credential_storage_password
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        archive_storage_path: provider_credential_archive_path(&state.config, provider)
            .map(|path| path.to_string_lossy().into_owned())
            .or_else(|| remote_location(provider, true)),
        archived_credential_count: archive_count.as_ref().ok().copied(),
        archive_storage_error: archive_count.err().map(|error| error.to_string()),
        archive_purge_supported,
        archive_purge_unsupported_reason: archive_purge_unsupported_reason.map(str::to_owned),
        storage_auth_configured: crate::credential_pool_storage::authentication_configured(
            provider.credential_storage_connection.as_ref(),
        ),
        archive_auth_configured: crate::credential_pool_storage::authentication_configured(
            provider.credential_archive_connection.as_ref(),
        ),
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

fn remote_location(provider: &ProviderConfigYaml, archive: bool) -> Option<String> {
    use crate::routing::config::CredentialStorageConnection;
    let connection = if archive {
        provider.credential_archive_connection.as_ref()
    } else {
        provider.credential_storage_connection.as_ref()
    }?;
    let namespace = crate::credential_pool_storage::namespace(connection, &provider.id, archive);
    match connection {
        CredentialStorageConnection::Webdav { endpoint, .. } => {
            Some(format!("{}/{namespace}/", endpoint.trim_end_matches('/')))
        }
        CredentialStorageConnection::S3 {
            endpoint, bucket, ..
        } => Some(format!(
            "{}/{bucket}/{namespace}/",
            endpoint.trim_end_matches('/')
        )),
        _ => None,
    }
}
