//! One admitted run owns its configuration, pools and explicit deletion intent.
use std::collections::HashSet;
use std::path::PathBuf;

use deadpool_redis::Pool;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::error::GatewayError;
use crate::state::{AppState, ProviderCredentialFolderSyncRuntime};

use super::export::export_database_credentials;
use super::import::import_folder_credentials;
use super::status::{
    get_owned_folder_sync_status, record_sync_run, CompletedSyncPhases, FolderSyncStatusConfig,
};
use super::{
    resolve_root_dir, FolderSyncCounters, FolderSyncDirection,
    ProviderCredentialFolderSyncStatusView,
};

mod owner;
#[cfg(test)]
mod tests;

struct RunInputs {
    pool: Pool,
    pg_pool: Option<PgPool>,
    config: FolderSyncStatusConfig,
    runtime: ProviderCredentialFolderSyncRuntime,
    root_dir: Result<PathBuf, GatewayError>,
    explicit_deleted_paths: HashSet<String>,
}

pub(super) async fn run_folder_sync_once_with_explicit_deletes(
    state: &AppState,
    direction: FolderSyncDirection,
    explicit_deleted_paths: &HashSet<String>,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    owner::run_owned(&state.provider_credential_folder_sync, || {
        let inputs = RunInputs {
            pool: state.redis_pool.clone(),
            pg_pool: state.pg_pool.clone(),
            config: FolderSyncStatusConfig::new(
                &state.config,
                state.provider_credential_folder_sync.enabled(),
            ),
            runtime: state.provider_credential_folder_sync.clone(),
            // Resolve the immutable configured spelling now; return errors only after status I/O.
            root_dir: resolve_root_dir(&state.config),
            explicit_deleted_paths: explicit_deleted_paths.clone(),
        };
        execute(inputs, direction)
    })
    .await
}

async fn execute(
    inputs: RunInputs,
    direction: FolderSyncDirection,
) -> Result<ProviderCredentialFolderSyncStatusView, GatewayError> {
    get_owned_folder_sync_status(&inputs.pool, &inputs.config, &inputs.runtime).await?;

    let root_dir = inputs.root_dir?;
    std::fs::create_dir_all(&root_dir).map_err(|error| {
        GatewayError::server_error(format!(
            "create provider credential sync root {}: {error}",
            root_dir.display()
        ))
    })?;

    let pg_pool = inputs
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))?;

    let mut counters = FolderSyncCounters::default();
    let mut completed = CompletedSyncPhases::default();
    let now = OffsetDateTime::now_utc();

    let result: Result<(), GatewayError> = async {
        if matches!(
            direction,
            FolderSyncDirection::Import | FolderSyncDirection::Both
        ) {
            if !inputs.config.import_enabled {
                return Err(GatewayError::bad_request(
                    "当前未启用服务商凭证文件夹导入模式",
                ));
            }
            import_folder_credentials(
                pg_pool,
                &inputs.pool,
                &root_dir,
                inputs.config.delete_missing,
                &inputs.explicit_deleted_paths,
                &mut counters,
            )
            .await?;
            completed.import = true;
        }
        if matches!(
            direction,
            FolderSyncDirection::Export | FolderSyncDirection::Both
        ) {
            if !inputs.config.export_enabled {
                return Err(GatewayError::bad_request(
                    "当前未启用服务商凭证文件夹导出模式",
                ));
            }
            export_database_credentials(
                pg_pool,
                &root_dir,
                inputs.config.delete_missing,
                &mut counters,
            )
            .await?;
            completed.export = true;
        }
        Ok(())
    }
    .await;

    let status = record_sync_run(
        &inputs.pool,
        &inputs.config,
        &inputs.runtime,
        now,
        &completed,
        &counters,
        result.as_ref().err(),
    )
    .await?;
    result?;
    Ok(status)
}
