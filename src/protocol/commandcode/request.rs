//! Command Code applies the shared lossless text/function request boundary.
use super::{invalid, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
pub(super) fn validate(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    crate::protocol::openai::text_tools_request::validate(req, invalid)
}
