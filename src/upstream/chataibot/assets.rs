//! URL and base64 output both require a bounded, credential-free, fully decoded PNG/JPEG.
use super::{failure, transport};
use crate::{
    error::GatewayError,
    upstream::public_assets::{self, Failure},
};

pub(super) async fn download(
    url: &str,
    deadline: tokio::time::Instant,
) -> Result<(String, Vec<u8>), GatewayError> {
    let (bytes, mime) = public_assets::download(url, deadline, 16 * 1024 * 1024)
        .await
        .map_err(error)?;
    let (bytes, actual) = public_assets::image::validate(bytes, &mime)
        .await
        .map_err(error)?;
    Ok((actual.into(), bytes))
}
pub(super) fn error(error: Failure) -> GatewayError {
    match error {
        Failure::Timeout => transport::timeout(),
        Failure::Target => failure(
            "chataibot_asset_target",
            "ChatAIBot 资产不是可准入的公网 HTTPS 目标。",
        ),
        Failure::Http(_) => failure(
            "chataibot_asset_http",
            "ChatAIBot 资产读取失败；不跟随跳转、不自动重提。",
        ),
        Failure::Network => failure(
            "chataibot_asset_network",
            "ChatAIBot 资产网络失败；连接细节已省略。",
        ),
        Failure::Limit => failure("chataibot_body_limit", "ChatAIBot 响应超过本地字节预算。"),
        Failure::Type => failure(
            "chataibot_asset_type",
            "ChatAIBot 资产为空、类型不匹配或图片无效。",
        ),
    }
}
