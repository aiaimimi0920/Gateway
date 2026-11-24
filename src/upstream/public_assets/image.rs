//! Shared PNG/JPEG full decode with bounded dimensions, allocations and CPU owners.
use super::Failure;
use std::sync::{Arc, LazyLock};
static DECODERS: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(2)));

pub(crate) async fn validate(
    bytes: Vec<u8>,
    mime: &str,
) -> Result<(Vec<u8>, &'static str), Failure> {
    let (bytes, mime, ()) = validate_with(bytes, mime, |_| Ok(())).await?;
    Ok((bytes, mime))
}

/// Product-specific checks run inside the same bounded decode owner, without decoding twice.
pub(crate) async fn validate_with<T: Send + 'static>(
    bytes: Vec<u8>,
    mime: &str,
    inspect: impl FnOnce(&image::DynamicImage) -> Result<T, Failure> + Send + 'static,
) -> Result<(Vec<u8>, &'static str, T), Failure> {
    let format = image::guess_format(&bytes).map_err(|_| Failure::Type)?;
    let actual = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        _ => return Err(Failure::Type),
    };
    if mime != actual && mime != "application/octet-stream" {
        return Err(Failure::Type);
    }
    let permit = DECODERS
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| Failure::Type)?;
    // Dropping a caller cannot abort blocking decoding; the permit stays with its bounded work.
    let (bytes, facts) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode().map_err(|_| Failure::Type)?;
        let facts = inspect(&decoded)?;
        Ok::<_, Failure>((bytes, facts))
    })
    .await
    .map_err(|_| Failure::Type)??;
    Ok((bytes, actual, facts))
}
