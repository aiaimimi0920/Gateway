use crate::error::GatewayError;
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use crate::state::AppState;
use std::collections::HashSet;
use std::sync::Arc;

use super::*;

pub(super) async fn deliver_refill_result(
    state: &Arc<AppState>,
    task: &CredentialRefillTaskRecord,
    delivery: CredentialRefillDeliveryInput,
) -> Result<DeliveryOutcome, GatewayError> {
    match delivery {
        CredentialRefillDeliveryInput::FolderSync { relative_paths } => {
            let credentials = folder_delivery::collect(state, task, relative_paths).await?;
            let (created_count, revision_id) =
                commit_refill_credentials(state, task, credentials).await?;
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::FolderSync,
                created_count,
                message: format!("已从该渠道存储目录导入 {created_count} 个凭证。"),
                revision_id: Some(revision_id),
            })
        }
        CredentialRefillDeliveryInput::GatewayPull { artifact_reference } => {
            let artifact_reference = normalize_required_text(
                &artifact_reference,
                "artifactReference",
                MAX_ARTIFACT_REFERENCE_LENGTH,
            )?;
            let credentials =
                crate::credential_pool_automation::collect_refill_credentials_from_driver(
                    state,
                    &task.provider_id,
                    &task.id,
                    task.requested_count,
                    &artifact_reference,
                )
                .await
                .map_err(|error| {
                    GatewayError::bad_request(error.to_string())
                        .with_code("credential_refill_driver_pull_failed")
                })?;
            let (created_count, revision_id) =
                commit_refill_credentials(state, task, credentials).await?;
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::GatewayPull,
                created_count,
                message: format!("Gateway 已通过可信驱动器拉取 {created_count} 个凭证。"),
                revision_id: Some(revision_id),
            })
        }
        CredentialRefillDeliveryInput::DirectCallback { credentials } => {
            let (created_count, revision_id) =
                commit_refill_credentials(state, task, credentials).await?;
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::DirectCallback,
                created_count,
                message: format!("补号程序已直接回传 {created_count} 个凭证。"),
                revision_id: Some(revision_id),
            })
        }
    }
}

async fn commit_refill_credentials(
    state: &Arc<AppState>,
    task: &CredentialRefillTaskRecord,
    credentials: Vec<ProviderCredentialYaml>,
) -> Result<(usize, String), GatewayError> {
    let provider_id = &task.provider_id;
    let requested_count = task.requested_count;
    if credentials.is_empty() {
        return Err(GatewayError::bad_request("补号结果没有包含任何凭证")
            .with_code("credential_refill_empty_delivery"));
    }
    if credentials.len() > requested_count {
        return Err(GatewayError::bad_request("补号结果数量超过任务请求数量")
            .with_code("credential_refill_delivery_exceeds_request"));
    }
    let runtime = state.route_config_runtime.as_ref().ok_or_else(|| {
        GatewayError::service_unavailable("路由配置运行时不可用")
            .with_code("credential_refill_route_runtime_unavailable")
    })?;
    for _ in 0..3 {
        let snapshot = state.route_config.snapshot();
        let mut document = snapshot.document().clone();
        let provider = document
            .providers
            .iter_mut()
            .find(|provider| provider.id == *provider_id)
            .ok_or_else(|| GatewayError::not_found("补号任务对应的 Provider 已不存在"))?;
        if task.trigger != CredentialRefillTrigger::UserRequested
            && (!provider.auto_refill_enabled
                || crate::credential_pool_automation::capacity::pool_min_size(provider) == 0)
        {
            return Err(GatewayError::conflict("该渠道已关闭自动补号")
                .with_code("credential_refill_automatic_disabled"));
        }
        let availability =
            crate::credential_pool_automation::availability::pool_availability(state, provider)
                .await?;
        let created_count = append_refill_credentials_with_capacity(
            provider,
            credentials.clone(),
            availability.remaining,
        )?;
        if task.trigger != CredentialRefillTrigger::UserRequested {
            provider.pool_refill_in_progress = true;
        }
        if availability.available
            >= crate::credential_pool_automation::capacity::pool_max_size(provider)
        {
            provider.pool_refill_in_progress = false;
        }
        crate::credential_pool_automation::capacity::normalize_refill_state(provider);
        if created_count == 0 {
            return Ok((0, snapshot.revision().id().to_string()));
        }
        if let Some(claim_token) = task.claim_token.as_deref() {
            // A long pull may outlive its worker lease; never commit an obsolete claim.
            require_claimed_task(state, &task.id, claim_token).await?;
        }
        match runtime
            .commit_automation_document(
                snapshot.revision().id(),
                document,
                Some(format!(
                    "credential refill delivery for {provider_id}: +{created_count}"
                )),
            )
            .await
        {
            Ok(committed) => {
                return Ok((created_count, committed.revision().id().to_string()));
            }
            Err(error) if error.code() == "console_revision_conflict" => continue,
            Err(error) => {
                return Err(GatewayError::server_error(error.to_string())
                    .with_code("credential_refill_route_commit_failed"));
            }
        }
    }
    Err(GatewayError::conflict("路由配置持续变化，补号结果暂未提交")
        .with_code("credential_refill_route_revision_conflict"))
}

#[cfg(test)]
pub(super) fn append_refill_credentials(
    provider: &mut ProviderConfigYaml,
    credentials: Vec<ProviderCredentialYaml>,
) -> Result<usize, GatewayError> {
    let capacity = crate::credential_pool_automation::capacity::remaining_capacity(provider);
    append_refill_credentials_with_capacity(provider, credentials, capacity)
}

pub(super) fn append_refill_credentials_with_capacity(
    provider: &mut ProviderConfigYaml,
    credentials: Vec<ProviderCredentialYaml>,
    capacity: usize,
) -> Result<usize, GatewayError> {
    let mut existing_ids = provider
        .credentials
        .iter()
        .filter_map(|credential| credential.id.clone())
        .collect::<HashSet<_>>();
    let mut additions = Vec::new();
    for credential in credentials {
        let id = credential
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| GatewayError::bad_request("补号结果中的凭证缺少 ID"))?;
        normalize_identifier(id, "credentialId", MAX_WORKER_ID_LENGTH)?;
        if existing_ids.insert(id.to_string()) {
            if !credential.enabled.unwrap_or(true) {
                return Err(GatewayError::bad_request("补号结果必须包含启用的可用凭证")
                    .with_code("credential_refill_disabled_credential"));
            }
            additions.push(credential);
        }
    }
    if additions.len() > capacity {
        return Err(GatewayError::conflict("补号结果超过凭证池当前剩余容量")
            .with_code("credential_refill_capacity_exceeded"));
    }
    let created_count = additions.len();
    provider.credentials.extend(additions);
    Ok(created_count)
}
