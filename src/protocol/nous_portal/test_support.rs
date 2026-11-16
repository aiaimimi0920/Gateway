//! No runtime override: this scoped remapping exists only inside the library test harness.
pub(crate) async fn origin<T>(origin: String, future: impl std::future::Future<Output = T>) -> T {
    super::transport::TEST_ORIGIN.scope(origin, future).await
}

pub(crate) fn transport_plan(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    plan: &crate::upstream::common::RequestPlan,
) -> Option<crate::upstream::common::RequestPlan> {
    if !super::business::owns_payload(payload) {
        return None;
    }
    let origin = super::transport::TEST_ORIGIN
        .try_with(Clone::clone)
        .expect("Nous sender tests require an explicit loopback scope");
    let target = url::Url::parse(&origin).expect("test origin");
    assert_eq!(target.scheme(), "http");
    assert_eq!(target.host_str(), Some("127.0.0.1"));
    assert!(target.port().is_some());
    let url = url::Url::parse(&plan.url).expect("validated product URL");
    Some(crate::upstream::common::RequestPlan {
        method: plan.method.clone(),
        url: format!("{origin}{}", url.path()),
        query: plan.query.clone(),
        body: plan.body.clone(),
        response_kind: plan.response_kind,
    })
}
