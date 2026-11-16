//! Existing cover-image outputs only; never download Udio music or video playback streams.
use super::browser_worker;
use crate::{
    error::GatewayError,
    protocol::{canonical::CanonicalRelayRequest, udio::UdioSong},
    upstream::{
        public_assets::{self, Failure},
        udio_response_helpers::{
            build_udio_downloaded_images_response, resolve_udio_image_generation_plan,
        },
    },
};

pub(super) async fn finalize(
    request: &CanonicalRelayRequest,
    prompt: &str,
    songs: &[UdioSong],
    deadline: tokio::time::Instant,
) -> Result<serde_json::Value, GatewayError> {
    let (urls, response) = resolve_udio_image_generation_plan(request, prompt, songs)?;
    // URL-only remains metadata without new I/O or claims of valid image content. DNS pinning
    // belongs to the actual downloader; validate all selected targets before the first read.
    for url in &urls {
        public_assets::checked_url(url).map_err(error)?;
    }
    if let Some(response) = response {
        return Ok(response);
    }
    // The existing request count is bounded to two images: at most 32 MiB retained raw bytes.
    // Base64 expansion stays bounded; the decoder owns its CPU permit even after cancellation.
    let mut images = Vec::with_capacity(urls.len());
    for url in urls {
        let (bytes, mime) = public_assets::download(&url, deadline, 16 * 1024 * 1024)
            .await
            .map_err(error)?;
        let (bytes, mime) = public_assets::image::validate(bytes, &mime)
            .await
            .map_err(error)?;
        images.push((mime.to_owned(), bytes));
    }
    Ok(build_udio_downloaded_images_response(
        request, prompt, &images,
    ))
}

fn error(failure: Failure) -> GatewayError {
    let (code, message) = match failure {
        Failure::Timeout => return browser_worker::timeout_error(),
        Failure::Target => (
            "udio_asset_target",
            "Udio 图片资产不是可准入的公网 HTTPS 目标。",
        ),
        Failure::Http(_) => (
            "udio_asset_http",
            "Udio 图片资产读取失败；不跟随跳转，不重新生成。",
        ),
        Failure::Network => (
            "udio_asset_network",
            "Udio 图片资产网络失败；连接细节已省略。",
        ),
        Failure::Limit => ("udio_asset_limit", "Udio 图片资产超过本地字节预算。"),
        Failure::Type => (
            "udio_asset_type",
            "Udio 图片资产为空、类型不匹配或无法完整解码为 PNG/JPEG。",
        ),
    };
    // Asset HTTP/auth failures do not prove the generation account is invalid. Signed URLs,
    // response bodies and arbitrary connection details must not enter the public error.
    let mut error = GatewayError::server_error(message).with_code(code);
    error.http_status = Some(502);
    browser_worker::no_replay(error)
}
