//! The saved MCP profile is management-only until a separately reviewed business executor exists.
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};

pub const ADAPTER: &str = "lumalabs_mcp_readonly";
pub const PROFILE: &str = "lumalabs_mcp_readonly";
pub const FAMILY: &str = "lumalabs_mcp";
pub const AUTH_MODE: &str = "luma_mcp_oauth";

pub fn owns_base(base: &str) -> bool {
    url::Url::parse(base).is_ok_and(|url| {
        url.host_str() == Some("agents.lumalabs.ai")
            && (url.path() == "/mcp" || url.path().starts_with("/mcp/"))
    })
}
pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    payload.adapter == ADAPTER
        || payload.auth_mode.as_deref() == Some(AUTH_MODE)
        || owns_base(&payload.base_url)
}
pub fn reject_generic(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    if owns_payload(payload) {
        return Err(invalid());
    }
    Ok(())
}
pub fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Luma MCP 账号仅支持专用只读工具目录检查，不能用于普通请求或模型测试。",
    )
    .with_code("luma_mcp_management_only")
}
