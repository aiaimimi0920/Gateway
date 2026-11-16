//! Web image assets have a separate credential-free transport and cannot trigger regeneration.
use super::browser_worker;
use crate::{
    error::GatewayError,
    upstream::public_assets::{self, Failure},
};

pub(super) async fn download(
    url: &str,
    deadline: tokio::time::Instant,
) -> Result<(Vec<u8>, &'static str), GatewayError> {
    let (bytes, mime) = public_assets::download(url, deadline, 16 * 1024 * 1024)
        .await
        .map_err(error)?;
    public_assets::image::validate(bytes, &mime)
        .await
        .map_err(error)
}

fn error(error: Failure) -> GatewayError {
    let (code, message) = match error {
        Failure::Timeout => return browser_worker::timeout_error(),
        Failure::Target => (
            "lumalabs_asset_target",
            "Luma Web 图片资产不是可准入的公网 HTTPS 目标。",
        ),
        Failure::Http(_) => (
            "lumalabs_asset_http",
            "Luma Web 图片资产读取失败；不跟随跳转，不重新生成。",
        ),
        Failure::Network => (
            "lumalabs_asset_network",
            "Luma Web 图片资产网络失败；连接细节已省略。",
        ),
        Failure::Limit => (
            "lumalabs_asset_limit",
            "Luma Web 图片资产超过本地字节预算。",
        ),
        Failure::Type => (
            "lumalabs_asset_type",
            "Luma Web 图片资产为空、类型不匹配或无法完整解码为 PNG/JPEG。",
        ),
    };
    // An asset 403 is not evidence that the generation credential is invalid. Do not expose
    // signed URLs, upstream bodies or DNS/connection details in this public projection.
    let mut error = GatewayError::server_error(message).with_code(code);
    error.http_status = Some(502);
    browser_worker::no_replay(error)
}
