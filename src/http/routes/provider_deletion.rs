//! Bounded ownership of ordered management file, database and Redis deletions.
use crate::error::GatewayError;
use crate::state::GatewayLifecycleState;

pub(super) async fn run_owned<T, F, Fut>(
    lifecycle: &GatewayLifecycleState,
    prepare: F,
) -> Result<T, GatewayError>
where
    T: Send + 'static,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, GatewayError>> + Send + 'static,
{
    // Both DELETE routes share admission before input copies or DB hydration.
    let permit = lifecycle.try_begin_provider_deletion().ok_or_else(|| {
        let mut error = GatewayError::conflict("a provider deletion is already running");
        error.code = Some("provider_management_delete_busy".to_string());
        error
    })?;
    crate::concurrency::dedicated_owner::run(
        permit,
        prepare(),
        provider_deletion_runtime_stopped,
        provider_deletion_task_failed,
    )
    .await
}

fn provider_deletion_runtime_stopped() -> GatewayError {
    GatewayError::service_unavailable("provider deletion runtime stopped")
        .with_code("provider_management_delete_runtime_stopped")
}

fn provider_deletion_task_failed() -> GatewayError {
    GatewayError::server_error("provider deletion operation task failed")
        .with_code("provider_management_delete_task_failed")
}

#[cfg(test)]
mod tests;
