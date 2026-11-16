//! Full Vertex binds shared REST tools/Parts to its own OAuth source, never Express state.
use crate::{
    error::GatewayError,
    protocol::canonical::CanonicalRelayResponse,
    upstream::{vertex_product::OAUTH as CONTRACT, vertex_response::VerifiedResponse},
};
#[cfg(test)]
use serde_json::Value;
pub(crate) async fn accumulate(
    request: impl std::future::Future<Output = Result<rquest::Response, GatewayError>>,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    project(CONTRACT.read(request).await?, model)
}
fn project(
    verified: VerifiedResponse,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    CONTRACT.project(verified, model)
}
#[cfg(test)]
mod tests;
