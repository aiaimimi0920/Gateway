//! Google Flow control plane is native HTTP. Generation has no browser/proof fallback.
#[cfg(test)]
mod live_tests;
mod session;
#[cfg(test)]
mod tests;
mod transport;
mod wire;

use crate::error::{FallbackHint, GatewayError};
use crate::routing::candidate::ProviderAccountPayload;
use std::time::Duration;

pub(crate) fn failure(code: &str, message: &str) -> GatewayError {
    let mut error = GatewayError::service_unavailable(message)
        .with_provider("flow_web_compatible")
        .with_code(code);
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: message.into(),
    };
    error
}

pub fn generation_unavailable() -> GatewayError {
    failure("flow_web_fresh_proof_unavailable",
        "Google Flow 图片/视频生成需要 fresh reCAPTCHA proof；当前未实现经真实验收的 HTTP-only 获取路线。未发送生成请求，不会启动浏览器或自动重试。")
}

pub async fn read_credits(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
) -> Result<u64, GatewayError> {
    if payload.base_url.trim_end_matches('/') != "https://flow.google.com" {
        return Err(failure(
            "flow_web_origin_invalid",
            "Flow 仅接受官方 HTTPS 地址。",
        ));
    }
    let key = payload
        .runtime_state_object_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| failure("flow_web_session_missing", "请先导入 Google Flow 账号。"))?;
    let bytes = crate::object_storage::gateway_object_storage()
        .map_err(|_| failure("flow_web_session_unavailable", "无法读取 Flow 凭据对象。"))?
        .read_bytes_with_limit(key, 32768)
        .await
        .map_err(|_| failure("flow_web_session_unavailable", "无法读取 Flow 凭据对象。"))?;
    read_credits_bundle(client, &bytes).await
}

pub(crate) async fn read_credits_bundle(
    client: &rquest::Client,
    bytes: &[u8],
) -> Result<u64, GatewayError> {
    verified_session(client, bytes)
        .await
        .map(|(credits, _)| credits)
}

pub(crate) async fn verify_import_bundle(
    client: &rquest::Client,
    bytes: &[u8],
) -> Result<Vec<u8>, GatewayError> {
    verified_session(client, bytes).await?.1.bundle_bytes()
}

async fn verified_session(
    client: &rquest::Client,
    bytes: &[u8],
) -> Result<(u64, session::FlowCookies), GatewayError> {
    let mut cookies = session::FlowCookies::from_bundle(bytes)?;
    let credits = tokio::time::timeout(
        Duration::from_secs(45),
        transport::read_credits(client, &mut cookies),
    )
    .await
    .map_err(|_| failure("flow_web_deadline_exceeded", "Flow HTTP 只读验证超时。"))??;
    Ok((credits, cookies))
}
