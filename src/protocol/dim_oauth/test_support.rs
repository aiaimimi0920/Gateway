//! In-process synthetic authority only; no override or fixture material is compiled into production.
pub(crate) fn context() -> serde_json::Value {
    serde_json::json!({"site":"china","client_id":"permitted-gateway",
        "requested_scopes":["openid","profile","email"],"granted_scopes":["openid","profile","email"],
        "token_endpoint":"https://dimagent.cn/token","userinfo_endpoint":"https://dimagent.cn/oauth/userinfo",
        "jwks_uri":"https://dimagent.cn/oauth/jwks.json","subject":"synthetic-subject",
        "user_id":"synthetic-user","assurance":"user_info_endpoint","nonce":"synthetic-nonce"})
}
pub(crate) async fn origin<T>(origin: String, future: impl std::future::Future<Output = T>) -> T {
    super::transport::TEST_ORIGIN.scope(origin, future).await
}
pub(crate) fn inherit_origin<F: std::future::Future>(
    future: F,
) -> impl std::future::Future<Output = F::Output> {
    let target = super::transport::TEST_ORIGIN.try_with(Clone::clone).ok();
    async move {
        match target {
            Some(target) => origin(target, future).await,
            None => future.await,
        }
    }
}

pub(crate) fn transport_plan(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    plan: &crate::upstream::common::RequestPlan,
) -> Option<crate::upstream::common::RequestPlan> {
    if !super::business::owns_payload(payload) {
        return None;
    }
    super::transport::TEST_ORIGIN
        .try_with(|_| ())
        .expect("DIM tests require explicit loopback scope");
    Some(crate::upstream::common::RequestPlan {
        method: plan.method.clone(),
        url: super::transport::test_target(&plan.url),
        query: plan.query.clone(),
        body: plan.body.clone(),
        response_kind: plan.response_kind,
    })
}
