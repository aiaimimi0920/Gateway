//! 仅测试可用的固定目标映射；生产验证完成后才改成非转发 loopback peer。
tokio::task_local! { static ORIGIN: String; }
pub(crate) async fn origin<T>(origin: String, future: impl std::future::Future<Output = T>) -> T {
    ORIGIN.scope(origin, future).await
}
pub(crate) fn test_origin() -> Option<String> {
    ORIGIN.try_with(Clone::clone).ok()
}
pub(crate) fn transport_plan(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    plan: &crate::upstream::common::RequestPlan,
) -> Option<crate::upstream::common::RequestPlan> {
    if !super::owns_payload(payload) {
        return None;
    }
    let origin = test_origin().expect("MiniMax test requires loopback scope");
    let url = url::Url::parse(&origin).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    let path = url::Url::parse(&plan.url).unwrap();
    Some(crate::upstream::common::RequestPlan {
        method: plan.method.clone(),
        url: format!("{origin}{}", path.path()),
        query: plan.query.clone(),
        body: plan.body.clone(),
        response_kind: plan.response_kind,
    })
}
