//! Final Key/header binding for both buffered and aggregated-SSE Gemini send paths.
use crate::{
    error::GatewayError, protocol::vertex_express as key,
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
    if !key::owns_payload(payload) {
        return Ok(());
    }
    key::validate_plan(payload, plan)?;
    // The caller cannot smuggle its Host, Cookie, OAuth bearer, query key or attribution headers.
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
        "x-goog-api-key",
        HeaderValue::from_str(&payload.api_key).map_err(|_| key::invalid())?,
    );
    *headers = bound;
    Ok(())
}
