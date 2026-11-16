//! DIM is a separate OAuth product; a matching model or email cannot redirect its bearer material.
use super::{site::atom, AuthorizationContext, Site};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};

pub const AUTH_MODE: &str = "dim_user_oauth";
pub fn owns_base(base: &str) -> bool {
    url::Url::parse(base)
        .is_ok_and(|url| matches!(url.host_str(), Some("dimagent.cn" | "dimagent.com")))
}
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.auth_mode.as_deref() == Some(AUTH_MODE) || owns_base(&payload.base_url)
}
pub fn base(site: Site) -> String {
    format!("{}/v1", site.issuer())
}
pub fn valid_token(token: &str) -> bool {
    atom(token, 32_768)
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid DIM OAuth product configuration.")
        .with_code("invalid_dim_user_oauth")
}
pub fn validate_context(
    payload: &ProviderAccountPayload,
    context: &AuthorizationContext,
) -> Result<(), GatewayError> {
    context.validate().map_err(|_| invalid())?;
    super::runtime::validate(payload, payload.canonical_adapter())?;
    if payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || payload.base_url.trim_end_matches('/') != base(context.site())
        || payload.canonical_adapter() != "openai_compatible"
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload
            .auth_header_name
            .as_deref()
            .is_some_and(|name| name != "authorization")
        || !payload.headers.is_empty()
        || !payload.discovered_protocols.is_empty()
        || !payload.api_key.is_empty() && !valid_token(&payload.api_key)
    {
        return Err(invalid());
    }
    Ok(())
}
