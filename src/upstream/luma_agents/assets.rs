//! Product errors and media policy wrap the shared untrusted public-asset transport.
mod validate;
use super::transport;
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::luma_agents::{failure, Media},
    upstream::public_assets::{self, Failure},
};
pub(super) async fn download(
    raw: &str,
    media: Media,
    deadline: tokio::time::Instant,
    limit: usize,
) -> Result<(Vec<u8>, &'static str), GatewayError> {
    let (bytes, mime) = public_assets::download(raw, deadline, limit)
        .await
        .map_err(error)?;
    validate::payload(bytes, &mime, media).await
}
fn error(error: Failure) -> GatewayError {
    match error {
        Failure::Target => unsafe_url(),
        Failure::Http(status) => failure(
            ErrorKind::ServerError,
            transport::public_failure_status(status),
            "luma_agents_asset_http",
            "Luma Agents 资产不可读取；未跟随跳转或重提生成。",
        ),
        Failure::Timeout => transport::timeout_error(),
        Failure::Limit => transport::body_limit(),
        Failure::Type => unreachable!("transport does not decode media"),
        Failure::Network => failure(
            ErrorKind::Network,
            502,
            "luma_agents_network",
            "Luma Agents 网络失败；不重提生成，连接详情已省略。",
        ),
    }
}
fn unsafe_url() -> GatewayError {
    failure(
        ErrorKind::ServerError,
        502,
        "luma_agents_asset_target",
        "Luma Agents 资产目标不是可准入的公网 HTTPS 地址。",
    )
}
#[cfg(test)]
fn checked_url(raw: &str) -> Result<url::Url, GatewayError> {
    public_assets::checked_url(raw).map_err(error)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn luma_agents_asset_targets_reject_local_schemes_credentials_and_redirect_inputs() {
        for url in [
            "http://example.com/a",
            "file:///tmp/a",
            "https://localhost:444/a",
            "https://127.0.0.1/a",
            "https://[::1]/a",
            "https://[::ffff:127.0.0.1]/a",
            "https://169.254.169.254/a",
            "https://10.0.0.1/a",
            "https://192.168.1.2/a",
            "https://user:secret@example.com/a",
            "https://example.com/a#secret",
        ] {
            assert!(checked_url(url).is_err(), "{url}");
        }
        assert!(checked_url("https://cdn.example.com/a?signature=opaque").is_ok());
    }
}
