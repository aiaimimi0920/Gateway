//! Verify only original playback URLs; never request billed downloads or repeat generation.
use super::transport;
use crate::{
    error::GatewayError,
    protocol::suno::SunoClip,
    upstream::public_assets::{self, Failure},
};
use std::collections::HashSet;

const PER_ASSET: usize = 32 * 1024 * 1024;
const TOTAL: usize = 64 * 1024 * 1024;

pub(super) async fn verify(
    clips: &[SunoClip],
    deadline: tokio::time::Instant,
) -> Result<(), GatewayError> {
    verify_with_limits(clips, deadline, PER_ASSET, TOTAL).await
}

pub(super) async fn verify_with_limits(
    clips: &[SunoClip],
    deadline: tokio::time::Instant,
    per_asset: usize,
    total: usize,
) -> Result<(), GatewayError> {
    // Remote worker metadata is untrusted too. Validate the entire set before the first GET.
    let mut ids = HashSet::new();
    if clips.is_empty()
        || clips.len() > 16
        || clips.iter().any(|c| {
            c.id.is_empty()
                || c.id.len() > 128
                || c.id.trim() != c.id
                || !ids.insert(&c.id)
                || !matches!(c.status.as_str(), "complete" | "completed")
                || c.audio_url
                    .as_ref()
                    .is_none_or(|u| public_assets::checked_url(u).is_err())
        })
    {
        return Err(error(Failure::Target));
    }
    let mut remaining = total;
    for clip in clips {
        let count = public_assets::audio::verify_url(
            clip.audio_url.as_deref().expect("validated audio URL"),
            deadline,
            per_asset.min(remaining),
        )
        .await
        .map_err(error)?;
        remaining -= count; // Reader proved count <= remaining; no cross-clip byte retention.
    }
    Ok(())
}

fn error(failure: Failure) -> GatewayError {
    let (code, message) = match failure {
        Failure::Timeout => return transport::timeout(),
        Failure::Target => (
            "suno_asset_target",
            "Suno 完成歌曲集合或音频目标无效；只允许原始公网 HTTPS 资产。",
        ),
        Failure::Http(_) => (
            "suno_asset_http",
            "Suno 音频资产读取失败；不跟随跳转，不重新生成。",
        ),
        Failure::Network => (
            "suno_asset_network",
            "Suno 音频资产网络失败；连接细节已省略。",
        ),
        Failure::Limit => (
            "suno_asset_limit",
            "Suno 音频超过字节、标签或解码时长预算。",
        ),
        Failure::Type => (
            "suno_asset_type",
            "Suno 音频类型不匹配、帧不完整或无法完整解码为 MP3。",
        ),
    };
    // A CDN 401/403 does not prove the generation credential expired. Never include signed
    // URLs, response bodies or decoder internals in the public error, and never regenerate.
    transport::failure(code, message)
}
