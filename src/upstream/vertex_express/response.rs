//! Express binds the shared Vertex response algorithms to its own product and ordered-turn identity.
use crate::{
    error::GatewayError, protocol::canonical::CanonicalRelayResponse,
    upstream::vertex_product::EXPRESS as CONTRACT,
};
#[cfg(test)]
use {
    crate::error::FallbackHint,
    serde_json::{json, Value},
};
pub(crate) async fn accumulate(
    request: impl std::future::Future<Output = Result<rquest::Response, GatewayError>>,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let verified = CONTRACT.read(request).await?;
    CONTRACT.project(verified, model)
}
#[cfg(test)]
fn parse(body: &[u8], model: &str) -> Result<CanonicalRelayResponse, GatewayError> {
    let verified = CONTRACT.parse(body)?;
    CONTRACT.project(verified, model)
}
#[cfg(test)]
mod tests;
