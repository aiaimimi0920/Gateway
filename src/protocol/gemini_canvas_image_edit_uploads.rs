//! Owned image-upload decoding and deterministic JPEG normalization.
use super::GeminiCanvasImageEditUpload;
use crate::error::GatewayError;
use crate::protocol::{canonical::CanonicalRelayRequest, gemini_business};
use base64::Engine;
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType};

pub fn extract_image_edit_uploads(
    req: &CanonicalRelayRequest,
) -> Result<Vec<GeminiCanvasImageEditUpload>, GatewayError> {
    normalize_image_edit_uploads(gemini_business::extract_uploads_from_request_body(
        &req.raw_body,
    )?)
}

pub(crate) fn normalize_image_edit_uploads(
    uploads: Vec<gemini_business::GeminiBusinessUpload>,
) -> Result<Vec<GeminiCanvasImageEditUpload>, GatewayError> {
    uploads
        .into_iter()
        .enumerate()
        .map(|(index, upload)| {
            let raw_bytes = base64::engine::general_purpose::STANDARD
                .decode(upload.base64_data.trim())
                .map_err(|error| {
                    GatewayError::bad_request(format!(
                        "Gemini Canvas image edit upload contained invalid base64 bytes: {error}"
                    ))
                    .with_code("invalid_image_edit_upload_base64")
                })?;
            let mime_type = upload.mime_type.trim().to_ascii_lowercase();
            if !mime_type.starts_with("image/") {
                return Err(GatewayError::bad_request(format!(
                    "Gemini Canvas image edit uploads require image mime types, got `{}`.",
                    upload.mime_type
                ))
                .with_code("invalid_image_edit_upload_mime_type"));
            }
            let bytes = normalize_image_edit_upload_to_jpeg(&mime_type, &raw_bytes)?;
            let file_name = if index == 0 {
                "edit-source.jpg".to_string()
            } else {
                format!("edit-source-{}.jpg", index + 1)
            };
            Ok(GeminiCanvasImageEditUpload {
                mime_type: "image/jpeg".to_string(),
                bytes,
                file_name,
                source_mime_type: mime_type,
                source_bytes: raw_bytes,
            })
        })
        .collect()
}

pub(super) fn normalize_image_edit_upload_to_jpeg(
    mime_type: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, GatewayError> {
    const TARGET_MAX_BYTES: usize = 127_600;
    const TARGET_MIN_SCALE: f32 = 0.18;
    const MAX_LONG_EDGE: u32 = 1400;
    const JPEG_QUALITY: u8 = 82;
    const MAX_JPEG_QUALITY: u8 = 86;
    const MAX_ATTEMPTS: usize = 12;

    let decoded = image::load_from_memory(bytes).map_err(|error| {
        GatewayError::bad_request(format!(
            "Gemini Canvas image edit upload bytes did not decode as an image: {error}"
        ))
        .with_code("invalid_image_edit_upload_image")
    })?;

    let (source_width, source_height) = (decoded.width(), decoded.height());
    let longest_edge = source_width.max(source_height);
    let base_image = if longest_edge > MAX_LONG_EDGE {
        let scale = MAX_LONG_EDGE as f32 / longest_edge as f32;
        let resized_width = ((source_width as f32 * scale).round() as u32).max(1);
        let resized_height = ((source_height as f32 * scale).round() as u32).max(1);
        decoded.resize_exact(
            resized_width,
            resized_height,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        decoded
    };

    let render_scaled = |scale: f32| -> image::DynamicImage {
        if (scale - 1.0).abs() < f32::EPSILON {
            base_image.clone()
        } else {
            let resized_width = ((base_image.width() as f32 * scale).round() as u32).max(1);
            let resized_height = ((base_image.height() as f32 * scale).round() as u32).max(1);
            base_image.resize_exact(
                resized_width,
                resized_height,
                image::imageops::FilterType::Lanczos3,
            )
        }
    };

    let full_encoded = encode_image_edit_upload_as_jpeg(&base_image, JPEG_QUALITY)?;
    if full_encoded.len() <= TARGET_MAX_BYTES {
        return Ok(full_encoded);
    }

    let mut low_scale = TARGET_MIN_SCALE;
    let mut high_scale = 1.0_f32;
    let mut best_under_limit: Option<(Vec<u8>, f32)> = None;
    let mut best_over_limit: Option<(Vec<u8>, f32)> = Some((full_encoded, 1.0_f32));

    for attempt in 0..MAX_ATTEMPTS {
        let next_scale = if attempt == 0 {
            ((TARGET_MAX_BYTES as f32 / best_over_limit.as_ref().unwrap().0.len() as f32).sqrt()
                * 0.995)
                .clamp(low_scale, high_scale)
        } else {
            ((low_scale + high_scale) / 2.0).clamp(TARGET_MIN_SCALE, 1.0)
        };

        let encoded = encode_image_edit_upload_as_jpeg(&render_scaled(next_scale), JPEG_QUALITY)?;
        let encoded_len = encoded.len();

        if encoded_len <= TARGET_MAX_BYTES {
            let should_replace = match &best_under_limit {
                Some((previous, _)) => encoded_len > previous.len(),
                None => true,
            };
            if should_replace {
                best_under_limit = Some((encoded, next_scale));
            }
            low_scale = next_scale;
        } else {
            let should_replace = match &best_over_limit {
                Some((previous, _)) => encoded_len < previous.len(),
                None => true,
            };
            if should_replace {
                best_over_limit = Some((encoded, next_scale));
            }
            high_scale = next_scale;
        }

        if (high_scale - low_scale) <= 0.01 {
            break;
        }
    }

    if let Some((best_bytes, best_scale)) = best_under_limit.as_mut() {
        let best_image = render_scaled(*best_scale);
        for quality in (JPEG_QUALITY + 1)..=MAX_JPEG_QUALITY {
            let encoded = encode_image_edit_upload_as_jpeg(&best_image, quality)?;
            let encoded_len = encoded.len();
            if encoded_len <= TARGET_MAX_BYTES && encoded_len > best_bytes.len() {
                *best_bytes = encoded;
            }
        }
    }

    best_under_limit
        .map(|(bytes, _)| bytes)
        .or_else(|| best_over_limit.map(|(bytes, _)| bytes))
        .ok_or_else(|| {
            GatewayError::server_error(format!(
            "Gemini Canvas image edit upload could not be normalized from mime type `{mime_type}`."
        ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_image_edit_normalize_failed")
        })
}

fn encode_image_edit_upload_as_jpeg(
    image: &image::DynamicImage,
    quality: u8,
) -> Result<Vec<u8>, GatewayError> {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    let mut encoded = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut encoded, quality);
    encoder
        .encode(&rgb, width, height, ExtendedColorType::Rgb8)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Gemini Canvas image edit upload JPEG transcode failed: {error}"
            ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_image_edit_jpeg_encode_failed")
        })?;
    Ok(encoded)
}
