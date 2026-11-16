//! Test-only loopback token peer; no configurable production token destination.
tokio::task_local! { pub(crate) static ENDPOINT: String; }
tokio::task_local! { pub(crate) static REGISTRATION_ENDPOINT: String; }

pub(crate) fn inherit_registration_endpoint<F: std::future::Future + Send>(
    future: F,
) -> impl std::future::Future<Output = F::Output> + Send {
    let endpoint = REGISTRATION_ENDPOINT.try_with(Clone::clone).ok();
    async move {
        match endpoint {
            Some(endpoint) => REGISTRATION_ENDPOINT.scope(endpoint, future).await,
            None => future.await,
        }
    }
}

pub(crate) fn inherit_endpoint<F: std::future::Future + Send>(
    future: F,
) -> impl std::future::Future<Output = F::Output> + Send {
    let endpoint = ENDPOINT.try_with(Clone::clone).ok();
    async move {
        match endpoint {
            Some(endpoint) => ENDPOINT.scope(endpoint, future).await,
            None => future.await,
        }
    }
}
