use crate::error::GatewayError;
use crate::provider_credential_folder_sync::{run_folder_sync_once, FolderSyncDirection};
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
            validate_folder_paths(&relative_paths)?;
            let status = run_folder_sync_once(state.as_ref(), FolderSyncDirection::Import).await?;
            let created_count = status.imported_count.saturating_add(status.updated_count);
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::FolderSync,
                created_count,
                message: format!("文件夹同步已完成，导入或更新 {created_count} 个凭证。"),
                revision_id: None,
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
            let (created_count, revision_id) = commit_refill_credentials(
                state,
                &task.provider_id,
                task.requested_count,
                credentials,
            )
            .await?;
            Ok(DeliveryOutcome {
                mode: CredentialRefillDeliveryMode::GatewayPull,
                created_count,
                message: format!("Gateway 已通过可信驱动器拉取 {created_count} 个凭证。"),
                revision_id: Some(revision_id),
            })
        }
        CredentialRefillDeliveryInput::DirectCallback { credentials } => {
            let (created_count, revision_id) = commit_refill_credentials(
                state,
                &task.provider_id,
                task.requested_count,
                credentials,
            )
            .await?;
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
    provider_id: &str,
    requested_count: usize,
    credentials: Vec<ProviderCredentialYaml>,
) -> Result<(usize, String), GatewayError> {
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
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| GatewayError::not_found("补号任务对应的 Provider 已不存在"))?;
        let created_count = append_refill_credentials(provider, credentials.clone())?;
        if created_count == 0 {
            return Ok((0, snapshot.revision().id().to_string()));
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

pub(super) fn append_refill_credentials(
    provider: &mut ProviderConfigYaml,
    credentials: Vec<ProviderCredentialYaml>,
) -> Result<usize, GatewayError> {
    let mut existing_ids = provider
        .credentials
        .iter()
        .filter_map(|credential| credential.id.clone())
        .collect::<HashSet<_>>();
    let mut created_count = 0usize;
    for credential in credentials {
        let id = credential
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| GatewayError::bad_request("补号结果中的凭证缺少 ID"))?;
        normalize_identifier(id, "credentialId", MAX_WORKER_ID_LENGTH)?;
        if existing_ids.insert(id.to_string()) {
            provider.credentials.push(credential);
            created_count += 1;
        }
    }
    Ok(created_count)
}
