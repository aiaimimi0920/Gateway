use regex::Regex;
use std::{collections::HashSet, sync::OnceLock};
use url::Url;

use super::{
    media_mime::{infer_image_mime_type, infer_mime_type},
    response_indicates_video_generation_quota_reached,
    stream_response::prioritize_music_assets,
    types::*,
};
use crate::error::GatewayError;

pub fn extract_page_blob_media_assets(
    body: &str,
    operation: GeminiCanvasMediaOperation,
) -> Result<Vec<GeminiCanvasMediaAsset>, GatewayError> {
    let normalized = normalize_page_blob_media_text(body);
    if operation == GeminiCanvasMediaOperation::Video
        && response_indicates_video_generation_quota_reached(&normalized)
    {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas video generation quota is currently exhausted.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_video_quota_reached"));
    }
    let download_urls = extract_google_media_asset_urls(&normalized);
    if download_urls.is_empty() {
        return Err(build_page_blob_media_missing_error(&[], operation));
    }

    let mut assets = Vec::new();
    let mut seen_urls = HashSet::new();
    for url in download_urls.into_iter().rev() {
        if !seen_urls.insert(url.clone()) {
            continue;
        }
        if let Some(asset) = extract_page_blob_media_asset(&url, operation) {
            assets.push(asset);
        }
    }
    if operation == GeminiCanvasMediaOperation::Music {
        prioritize_music_assets(&mut assets);
    }

    if assets.is_empty() {
        return Err(build_page_blob_media_missing_error(
            &seen_urls.into_iter().collect::<Vec<_>>(),
            operation,
        ));
    }

    Ok(assets)
}

fn extract_page_blob_media_asset(
    url: &str,
    operation: GeminiCanvasMediaOperation,
) -> Option<GeminiCanvasMediaAsset> {
    let mime_type = match operation {
        GeminiCanvasMediaOperation::Image => infer_image_mime_type(url),
        GeminiCanvasMediaOperation::Music | GeminiCanvasMediaOperation::Video => {
            infer_mime_type(url, "")
        }
    };
    let (kind, mime_type) = match operation {
        GeminiCanvasMediaOperation::Image if mime_type.starts_with("image/") => {
            ("image".to_string(), mime_type)
        }
        GeminiCanvasMediaOperation::Music if mime_type.starts_with("audio/") => {
            ("audio".to_string(), mime_type)
        }
        GeminiCanvasMediaOperation::Music if mime_type.starts_with("video/") => {
            ("video".to_string(), mime_type)
        }
        GeminiCanvasMediaOperation::Video if mime_type.starts_with("video/") => {
            ("video".to_string(), mime_type)
        }
        _ => return None,
    };

    Some(GeminiCanvasMediaAsset {
        kind,
        url: url.to_string(),
        mime_type,
        download_token: None,
        body_base64: None,
        alt: None,
        width: None,
        height: None,
        duration_seconds: None,
    })
}

fn build_page_blob_media_missing_error(
    download_urls: &[String],
    operation: GeminiCanvasMediaOperation,
) -> GatewayError {
    let code = match operation {
        GeminiCanvasMediaOperation::Image => "gemini_canvas_page_missing_image_asset",
        GeminiCanvasMediaOperation::Music => "gemini_canvas_page_missing_music_asset",
        GeminiCanvasMediaOperation::Video => "gemini_canvas_page_missing_video_asset",
    };
    let candidate_filenames = download_urls
        .iter()
        .take(6)
        .filter_map(|url| extract_download_filename_from_url(url))
        .collect::<Vec<_>>();
    let filenames_preview = if candidate_filenames.is_empty() {
        "<none>".to_string()
    } else {
        candidate_filenames.join("|")
    };

    let mut error =
        GatewayError::server_error("Gemini Canvas page blob did not include a usable media asset.")
            .with_provider("gemini_canvas_compatible")
            .with_code(code);
    error.message = format!(
        "{}; download_candidate_count={}; candidate_filenames={}",
        error.message,
        download_urls.len(),
        filenames_preview
    );
    error
}

fn extract_download_filename_from_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    for (key, value) in parsed.query_pairs() {
        if key == "filename" {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    parsed
        .path_segments()
        .and_then(|segments| segments.last())
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
}

fn extract_google_media_asset_urls(body: &str) -> Vec<String> {
    static GOOGLE_MEDIA_URL_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = GOOGLE_MEDIA_URL_REGEX.get_or_init(|| {
        Regex::new(
            r#"https?://(?:contribution\.usercontent\.google\.com/download\?[^"'<>\\\s]+|(?:lh3\.googleusercontent\.com|work\.fife\.usercontent\.google\.com)/(?:gg(?:-dl)?|rd-gg-dl|rd-ogw)/[^"'<>\\\s]+)"#,
        )
        .expect("google media asset url regex must compile")
    });

    regex
        .find_iter(body)
        .map(|matched| matched.as_str().trim().to_string())
        .collect()
}

fn normalize_page_blob_media_text(body: &str) -> String {
    body.replace("\\u003d", "=")
        .replace("\\u0026", "&")
        .replace("\\u003a", ":")
        .replace("\\u002f", "/")
        .replace("\\u003f", "?")
        .replace("\\u0025", "%")
        .replace("\\/", "/")
        .replace("&amp;", "&")
}
