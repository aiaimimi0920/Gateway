//! Shared full image decode; MP4 remains a product-level structure check, not playback validation.
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::luma_agents::{failure, Media},
};
pub(super) async fn payload(
    bytes: Vec<u8>,
    mime: &str,
    media: Media,
) -> Result<(Vec<u8>, &'static str), GatewayError> {
    if media == Media::Video {
        if !matches!(mime, "video/mp4" | "application/octet-stream") || !mp4(&bytes) {
            return Err(invalid());
        }
        return Ok((bytes, "video/mp4"));
    }
    crate::upstream::public_assets::image::validate(bytes, mime)
        .await
        .map_err(|_| invalid())
}
fn invalid() -> GatewayError {
    failure(
        ErrorKind::ServerError,
        502,
        "luma_agents_asset_type",
        "Luma Agents 资产为空、媒体类型不匹配或载荷无效。",
    )
}
fn boxes(mut bytes: &[u8]) -> Option<Vec<(&[u8], &[u8])>> {
    let mut result = Vec::new();
    while !bytes.is_empty() {
        if bytes.len() < 8 || result.len() >= 4096 {
            return None;
        }
        let small = u32::from_be_bytes(bytes[..4].try_into().ok()?) as usize;
        let (size, header) = match small {
            0 => (bytes.len(), 8),
            1 if bytes.len() >= 16 => (
                usize::try_from(u64::from_be_bytes(bytes[8..16].try_into().ok()?)).ok()?,
                16,
            ),
            1 => return None,
            size => (size, 8),
        };
        if size < header || size > bytes.len() {
            return None;
        }
        result.push((&bytes[4..8], &bytes[header..size]));
        bytes = &bytes[size..];
    }
    Some(result)
}
fn mp4(bytes: &[u8]) -> bool {
    let Some(top) = boxes(bytes) else {
        return false;
    };
    let ftyp = top.iter().any(|(kind, body)| {
        *kind == b"ftyp"
            && body.len() >= 8
            && (body[..4] == *b"isom"
                || body[..4] == *b"mp41"
                || body[..4] == *b"mp42"
                || body[..4] == *b"avc1")
    });
    let mdat = top
        .iter()
        .any(|(kind, body)| *kind == b"mdat" && !body.is_empty());
    let video = top
        .iter()
        .filter(|(k, _)| *k == b"moov")
        .any(|(_, body)| has_video(body));
    ftyp && mdat && video
}
fn has_video(body: &[u8]) -> bool {
    let Some(moov) = boxes(body) else {
        return false;
    };
    for (_, trak) in moov.into_iter().filter(|(k, _)| *k == b"trak") {
        let Some(trak) = boxes(trak) else {
            return false;
        };
        for (_, mdia) in trak.into_iter().filter(|(k, _)| *k == b"mdia") {
            let Some(mdia) = boxes(mdia) else {
                return false;
            };
            if mdia
                .into_iter()
                .any(|(k, b)| k == b"hdlr" && b.len() >= 12 && &b[8..12] == b"vide")
            {
                return true;
            }
        }
    }
    false
}
