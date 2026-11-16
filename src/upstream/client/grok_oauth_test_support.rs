//! Test-only remapping occurs after final product validation; no environment or release escape hatch.
use crate::{routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan};
use std::future::Future;
tokio::task_local! { static ORIGIN: String; }
pub(super) async fn within<T>(origin: &str, future: impl Future<Output = T>) -> T {
    let url = url::Url::parse(origin).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    crate::request_wait::transport::within(false, ORIGIN.scope(origin.to_owned(), future)).await
}
pub(super) fn transport_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Option<RequestPlan> {
    if !crate::protocol::grok_oauth::business::owns_payload(payload) {
        return None;
    }
    Some(
        ORIGIN
            .try_with(|origin| RequestPlan {
                method: plan.method.clone(),
                url: format!("{origin}/v1/responses"),
                query: plan.query.clone(),
                body: plan.body.clone(),
                response_kind: plan.response_kind,
            })
            .expect("Grok tests may only send within an explicit loopback scope"),
    )
}
