//! CPU/allocation ownership for untrusted inline PNG/JPEG; never a URL fetch.
use super::{invalid, Images};
use crate::error::GatewayError;
use std::sync::{Arc, LazyLock};
static DECODERS: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(2)));

pub(super) async fn validate(images: Images) -> Result<(), GatewayError> {
    if images.is_empty() {
        return Ok(());
    }
    // No queue of callers retaining decoded bytes behind these CPU slots.
    let permit = DECODERS
        .clone()
        .try_acquire_owned()
        .map_err(|_| crate::protocol::cursor_remote::error("image_decode_busy"))?;
    // Cancellation drops the waiter, not a running blocking decoder. Retain its
    // permit until the work exits so repeated cancellation cannot bypass the cap.
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        for image in images.0 {
            let format = if image.mime == "image/png" {
                static_png(&image.bytes)?;
                image::ImageFormat::Png
            } else {
                image::ImageFormat::Jpeg
            };
            let reader = || {
                let mut reader =
                    image::ImageReader::with_format(std::io::Cursor::new(&*image.bytes), format);
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(4096);
                limits.max_image_height = Some(4096);
                limits.max_alloc = Some(32 * 1024 * 1024);
                reader.limits(limits);
                reader
            };
            let (width, height) = reader().into_dimensions().map_err(|_| invalid())?;
            if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 4 * 1024 * 1024 {
                return Err(invalid());
            }
            reader().decode().map_err(|_| invalid())?;
        }
        Ok(())
    })
    .await
    .map_err(|_| invalid())?
}

fn static_png(bytes: &[u8]) -> Result<(), GatewayError> {
    let mut offset = 8;
    while let Some(header) = bytes.get(offset..offset + 8) {
        let size = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let kind = &header[4..];
        offset = offset
            .checked_add(size)
            .and_then(|v| v.checked_add(12))
            .filter(|v| *v <= bytes.len())
            .ok_or_else(invalid)?;
        if kind == b"acTL" || kind == b"fcTL" || kind == b"fdAT" {
            return Err(invalid());
        }
        if kind == b"IEND" {
            return if size == 0 && offset == bytes.len() {
                Ok(())
            } else {
                Err(invalid())
            };
        }
    }
    Err(invalid())
}
