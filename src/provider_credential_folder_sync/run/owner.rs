//! Admission and task lifetime shared by manual, watcher and refill runs.
use crate::error::GatewayError;
use crate::state::ProviderCredentialFolderSyncRuntime;

pub(super) async fn run_owned<T, F, Fut>(
    runtime: &ProviderCredentialFolderSyncRuntime,
    prepare: F,
) -> Result<T, GatewayError>
where
    T: Send + 'static,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, GatewayError>> + Send + 'static,
{
    // Fail before input copies or queueing. Clones of the same runtime share capacity.
    let permit = runtime.try_begin_sync_run().ok_or_else(|| {
        let mut error = GatewayError::conflict("folder synchronization is already running");
        error.code = Some("provider_credential_folder_sync_run_busy".to_string());
        error
    })?;
    crate::concurrency::dedicated_owner::run(
        permit,
        prepare(),
        folder_sync_runtime_stopped,
        folder_sync_task_failed,
    )
    .await
}

fn folder_sync_runtime_stopped() -> GatewayError {
    GatewayError::service_unavailable("folder sync runtime stopped")
        .with_code("provider_credential_folder_sync_run_runtime_stopped")
}

fn folder_sync_task_failed() -> GatewayError {
    GatewayError::server_error("folder sync run operation task failed")
        .with_code("provider_credential_folder_sync_run_task_failed")
}

#[cfg(test)]
mod tests;
