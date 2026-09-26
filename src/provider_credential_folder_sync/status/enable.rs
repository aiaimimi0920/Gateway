use crate::error::GatewayError;
use crate::state::AppState;

use super::config::FolderSyncStatusConfig;
use super::store::update_folder_sync_enabled;
use super::{folder_sync_root_available, ProviderCredentialFolderSyncStatusView};

pub async fn set_runtime_enabled(
    state: &AppState,
    enabled: bool,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    if enabled && !folder_sync_root_available(&state.config) {
        return Err(GatewayError::service_unavailable(
            "未配置服务商凭证文件夹同步根目录",
        ));
    }
    // Admission precedes owned inputs and work; cancelled waiters have no effects.
    let permit = state
        .provider_credential_folder_sync
        .begin_enable_update()
        .await;
    let pool = state.redis_pool.clone();
    let runtime = state.provider_credential_folder_sync.clone();
    let config = FolderSyncStatusConfig::new(&state.config, enabled);
    // Dropping the caller detaches admitted work; its permit lives through status I/O.
    tokio::spawn(async move {
        let _permit = permit;
        let result: Result<ProviderCredentialFolderSyncStatusView, GatewayError> = async {
            let status = update_folder_sync_enabled(&pool, config.enabled, |status| {
                config.clone().apply(status)
            })
            .await?;
            runtime.set_enabled(config.enabled);
            Ok(status)
        }
        .await;
        if let Err(error) = &result {
            tracing::warn!(kind = ?error.kind, "folder sync enable operation failed");
        }
        result
    })
    .await
    .map_err(|_| GatewayError::server_error("folder sync enable operation task failed"))?
}
