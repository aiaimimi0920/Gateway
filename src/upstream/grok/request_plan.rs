use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::grok;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    if crate::upstream::grok_web::owns_payload(payload) {
        return Err(crate::upstream::grok_web::unsupported_plan());
    }
    grok::build_request_plan(payload, req, model)
}
