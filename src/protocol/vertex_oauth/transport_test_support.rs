//! Task-local synthetic peer and shortened budgets; never selectable by production configuration.
use std::{future::Future, sync::Arc, time::Duration};
use tokio::sync::Notify;

tokio::task_local! {
    pub(super) static ENDPOINT: String;
    pub(super) static BUDGET: Duration;
    pub(super) static BODY_STARTED: Arc<Notify>;
}

pub(crate) async fn test_endpoint<F: Future>(endpoint: String, future: F) -> F::Output {
    let url = url::Url::parse(&endpoint).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some_and(|port| port != 0));
    assert!(url.username().is_empty() && url.password().is_none());
    assert!(url.query().is_none() && url.fragment().is_none());
    ENDPOINT.scope(endpoint, future).await
}

/// Capture before spawning: Tokio task locals are not inherited by listener tasks.
pub(crate) fn inherit_endpoint<F: Future>(future: F) -> impl Future<Output = F::Output> {
    // Absent fixture configuration must never allow a spawned test to reach Google.
    let endpoint = ENDPOINT
        .try_with(Clone::clone)
        .unwrap_or_else(|_| "http://127.0.0.1:1/no-test-issuer".into());
    async move { ENDPOINT.scope(endpoint, future).await }
}

pub(super) async fn test_budget<F: Future>(budget: Duration, future: F) -> F::Output {
    assert!(budget <= Duration::from_secs(15));
    BUDGET.scope(budget, future).await
}

pub(super) async fn test_body_started<F: Future>(notify: Arc<Notify>, future: F) -> F::Output {
    BODY_STARTED.scope(notify, future).await
}
