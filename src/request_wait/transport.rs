//! Request-local policy is never serialized or placed in upstream headers/body.
tokio::task_local! { static MANAGED: bool; }
pub(crate) fn managed() -> bool {
    MANAGED.try_with(|enabled| *enabled).unwrap_or(false)
}
pub(crate) async fn within<T>(enabled: bool, future: impl std::future::Future<Output = T>) -> T {
    MANAGED.scope(enabled, future).await
}
