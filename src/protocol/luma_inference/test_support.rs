//! Test-only transport substitution occurs after the complete production plan/header checks.
tokio::task_local! { static ORIGIN: String; }
pub(crate) async fn origin<T>(origin: String, future: impl std::future::Future<Output = T>) -> T {
    ORIGIN.scope(origin, future).await
}
pub(crate) fn transport_plan(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    plan: &crate::upstream::common::RequestPlan,
) -> Option<crate::upstream::common::RequestPlan> {
    if !super::owns_payload(payload) {
        return None;
    }
    let origin = ORIGIN
        .try_with(Clone::clone)
        .expect("Luma inference test requires loopback peer");
    let url = url::Url::parse(&origin).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    Some(crate::upstream::common::RequestPlan {
        method: plan.method.clone(),
        url: format!("{origin}/v1/chat/completions"),
        query: plan.query.clone(),
        body: plan.body.clone(),
        response_kind: plan.response_kind,
    })
}
