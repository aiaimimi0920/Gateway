//! Full Vertex has its own task-local seam; the peer is a shared non-forwarding synthetic server.
pub(super) use super::vertex_express_test_support::Server;
use crate::{routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan};
tokio::task_local! { static ORIGIN: String; }
pub(super) async fn within<T>(origin: &str, future: impl std::future::Future<Output = T>) -> T {
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
    if !crate::protocol::vertex_oauth::business::owns_payload(payload) {
        return None;
    }
    crate::protocol::vertex_oauth::business::validate_plan(payload, plan).unwrap();
    Some(
        ORIGIN
            .try_with(|origin| RequestPlan {
                method: plan.method.clone(),
                url: format!("{origin}{}", url::Url::parse(&plan.url).unwrap().path()),
                query: plan.query.clone(),
                body: plan.body.clone(),
                response_kind: plan.response_kind,
            })
            .expect("Full Vertex tests must explicitly select loopback transport"),
    )
}
