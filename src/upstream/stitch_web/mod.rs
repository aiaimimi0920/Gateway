//! Independent Stitch Web UI-design owner. Runtime calls are bounded native HTTP only.
mod execution;
mod result;
mod session;
#[cfg(test)]
mod tests;
mod transport;
mod wire;
#[cfg(test)]
mod wire_tests;

use crate::error::{FallbackHint, GatewayError};
use crate::routing::candidate::ProviderAccountPayload;
pub use execution::execute;
pub const ORIGIN: &str = "https://stitch.withgoogle.com";
pub const MODEL: &str = "stitch-design";

pub(super) fn failure(code: &str, message: &str) -> GatewayError {
    let mut error = GatewayError::service_unavailable(message)
        .with_provider("stitch_web_compatible")
        .with_code(code);
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Stitch project creation/generation cannot be safely replayed.".into(),
    };
    error
}

pub(super) fn admit_origin(base: &str) -> Result<(), GatewayError> {
    if base.trim_end_matches('/') != ORIGIN {
        return Err(failure(
            "stitch_web_origin_invalid",
            "Stitch 仅接受官方 HTTPS 根地址。",
        ));
    }
    Ok(())
}

async fn material(payload: &ProviderAccountPayload) -> Result<Vec<u8>, GatewayError> {
    admit_origin(&payload.base_url)?;
    let key = payload
        .runtime_state_object_key
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| failure("stitch_web_session_missing", "请先导入 Stitch 账号。"))?;
    crate::object_storage::gateway_object_storage()
        .map_err(|_| {
            failure(
                "stitch_web_session_unavailable",
                "无法读取 Stitch 私有凭据对象。",
            )
        })?
        .read_bytes_with_limit(key, 32768)
        .await
        .map_err(|_| {
            failure(
                "stitch_web_session_unavailable",
                "无法读取 Stitch 私有凭据对象。",
            )
        })
}

pub async fn probe(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
    let bytes = material(payload).await?;
    verify_import_bundle(client, &bytes).await.map(|_| ())
}

pub(crate) async fn verify_import_bundle(
    client: &rquest::Client,
    bytes: &[u8],
) -> Result<Vec<u8>, GatewayError> {
    tokio::time::timeout(std::time::Duration::from_secs(45), async {
        let http = transport::StitchHttp::open(client, session::StitchCookies::from_bundle(bytes)?)
            .await?;
        http.cookies.bundle_bytes()
    })
    .await
    .map_err(|_| {
        failure(
            "stitch_web_deadline_exceeded",
            "Stitch HTTP 登录态预检超时。",
        )
    })?
}
