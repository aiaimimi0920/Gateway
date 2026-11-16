//! Final product-bound headers use the current access token, never legacy auth_token or caller auth.
use crate::{
    error::GatewayError, protocol::vertex_oauth::business,
    routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan,
};
use rquest::header::{HeaderMap, HeaderValue};
mod response;
pub(crate) use response::accumulate;
pub fn finalize_headers(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut HeaderMap,
) -> Result<(), GatewayError> {
    if !business::owns_payload(payload) {
        return Ok(());
    }
    business::validate_plan(payload, plan)?;
    let mut bound = HeaderMap::new();
    bound.insert("content-type", HeaderValue::from_static("application/json"));
    bound.insert(
        "accept",
        HeaderValue::from_static(if plan.url.ends_with(":streamGenerateContent") {
            "text/event-stream"
        } else {
            "application/json"
        }),
    );
    bound.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", payload.api_key))
            .map_err(|_| business::invalid())?,
    );
    if let Some(quota) = business::target(payload)?.quota_project() {
        bound.insert(
            "x-goog-user-project",
            HeaderValue::from_str(quota).map_err(|_| business::invalid())?,
        );
    }
    *headers = bound;
    Ok(())
}
