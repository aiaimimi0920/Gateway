use crate::protocol::gemini_canvas;
use crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserMediaAsset;

#[cfg(test)]
pub(crate) fn extract_gemini_canvas_direct_http_image_handoff_url(body: &str) -> Option<String> {
    let normalized = body
        .replace("\\u003d", "=")
        .replace("\\u0026", "&")
        .replace("\\u003a", ":")
        .replace("\\u002f", "/")
        .replace("\\u003f", "?")
        .replace("\\u0025", "%")
        .replace("\\/", "/");
    let mut search_from = 0usize;
    while let Some(offset) = normalized[search_from..].find("http") {
        let start = search_from + offset;
        let rest = &normalized[start..];
        if !(rest.starts_with("https://") || rest.starts_with("http://")) {
            search_from = start + "http".len();
            continue;
        }
        let end = rest
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '\\' | ')' | ']' | '}')
            })
            .unwrap_or(rest.len());
        let candidate = rest[..end]
            .trim_end_matches(|c: char| c == ',' || c == ';')
            .trim_end_matches('.');
        let lowered = candidate.to_ascii_lowercase();
        let looks_like_image_handoff = lowered.contains("rd-gg-dl")
            || lowered.contains("gg-dl")
            || lowered.contains("googleusercontent")
            || lowered.contains("work.fife")
            || lowered.contains(".png")
            || lowered.contains(".jpg")
            || lowered.contains(".jpeg")
            || lowered.contains(".webp");
        if looks_like_image_handoff {
            return Some(candidate.to_string());
        }
        search_from = start + "http".len();
    }

    let host_prefixed_prefixes = [
        "//lh3.googleusercontent.com/",
        "//work.fife.usercontent.google.com/",
        "lh3.googleusercontent.com/",
        "work.fife.usercontent.google.com/",
        "/rd-gg-dl/",
        "/gg-dl/",
        "/rd-ogw/",
    ];
    for prefix in host_prefixed_prefixes {
        if let Some(offset) = normalized.find(prefix) {
            let rest = &normalized[offset..];
            let end = rest
                .find(|c: char| {
                    c.is_whitespace()
                        || matches!(c, '"' | '\'' | '<' | '>' | '\\' | ')' | ']' | '}')
                })
                .unwrap_or(rest.len());
            let candidate = rest[..end]
                .trim_end_matches(|c: char| c == ',' || c == ';')
                .trim_end_matches('.');
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

pub(crate) fn should_forward_gemini_canvas_download_cookies(request_url: &str) -> bool {
    url::Url::parse(request_url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .map(|host| {
            let host = host.to_ascii_lowercase();
            host == "gemini.google.com"
                || host.ends_with(".google.com")
                || host.ends_with(".googleusercontent.com")
                || host.ends_with(".usercontent.google.com")
        })
        .unwrap_or(false)
}

pub(crate) fn resolve_relative_url(base_url: &str, location: &str) -> Option<String> {
    let location = location.trim();
    if location.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(base_url).ok()?;
    parsed.join(location).ok().map(|value| value.to_string())
}

pub(crate) fn normalize_gemini_canvas_direct_http_asset_url(
    reference_url: Option<&str>,
    candidate: &str,
) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = url::Url::parse(trimmed) {
        return Some(parsed.to_string());
    }
    if let Some(protocol_relative) = trimmed.strip_prefix("//") {
        return url::Url::parse(&format!("https://{protocol_relative}"))
            .ok()
            .map(|value| value.to_string());
    }
    for host_prefix in [
        "lh3.googleusercontent.com/",
        "work.fife.usercontent.google.com/",
        "contribution.usercontent.google.com/download?",
    ] {
        if trimmed.starts_with(host_prefix) {
            return url::Url::parse(&format!("https://{trimmed}"))
                .ok()
                .map(|value| value.to_string());
        }
    }
    if let Some(reference_url) = reference_url {
        if let Some(joined) = resolve_relative_url(reference_url, trimmed) {
            return Some(joined);
        }
    }
    None
}

pub(crate) fn gemini_canvas_asset_url_is_caller_usable(url: &str) -> bool {
    let trimmed = url.trim();
    (trimmed.starts_with("https://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("data:image/"))
        && !trimmed.is_empty()
}

pub(crate) fn sniff_image_mime_type_from_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some("image/jpeg");
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    None
}

pub(crate) fn infer_gemini_canvas_media_mime_type(
    content_type: Option<&str>,
    asset_mime_hint: Option<&str>,
    url: &str,
    asset_kind_hint: Option<&str>,
) -> String {
    let content_type = content_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    if let Some(value) = content_type.as_deref() {
        if value.starts_with("audio/") || value.starts_with("video/") {
            return value.to_string();
        }
    }

    let asset_mime_hint = asset_mime_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    if let Some(value) = asset_mime_hint.as_deref() {
        if value.starts_with("audio/") || value.starts_with("video/") {
            return value.to_string();
        }
    }

    let lowered = url.to_ascii_lowercase();
    if lowered.contains(".mp3") {
        return "audio/mpeg".to_string();
    }
    if lowered.contains(".wav") {
        return "audio/wav".to_string();
    }
    if lowered.contains(".ogg") || lowered.contains(".opus") {
        return "audio/ogg".to_string();
    }
    if lowered.contains(".m4a") {
        return "audio/mp4".to_string();
    }
    if lowered.contains(".mp4") {
        return "video/mp4".to_string();
    }
    match asset_kind_hint.map(|value| value.trim().to_ascii_lowercase()) {
        Some(kind) if kind == "audio" => "audio/mpeg".to_string(),
        _ => "video/mp4".to_string(),
    }
}

pub(crate) fn infer_gemini_canvas_media_kind(
    mime_type: &str,
    asset_kind_hint: Option<&str>,
    url: &str,
) -> String {
    let lowered_mime = mime_type.trim().to_ascii_lowercase();
    if lowered_mime.starts_with("audio/") {
        return "audio".to_string();
    }
    if lowered_mime.starts_with("video/") {
        return "video".to_string();
    }
    if let Some(kind) = asset_kind_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
    {
        return kind;
    }
    if url.to_ascii_lowercase().contains(".mp3") {
        "audio".to_string()
    } else {
        "video".to_string()
    }
}

pub(crate) fn convert_gemini_canvas_asset(
    asset: &GeminiCanvasBrowserMediaAsset,
) -> gemini_canvas::GeminiCanvasMediaAsset {
    gemini_canvas::GeminiCanvasMediaAsset {
        kind: asset.kind.clone(),
        url: asset.url.clone(),
        mime_type: asset.mime_type.clone(),
        download_token: None,
        body_base64: asset.body_base64.clone(),
        alt: asset.alt.clone(),
        width: asset.width,
        height: asset.height,
        duration_seconds: asset.duration_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_browser_media_asset() -> GeminiCanvasBrowserMediaAsset {
        GeminiCanvasBrowserMediaAsset {
            kind: "image".to_string(),
            url: "blob:https://gemini.google.com/example".to_string(),
            mime_type: "image/jpeg".to_string(),
            body_base64: Some("aGVsbG8=".to_string()),
            alt: Some("preview".to_string()),
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
        }
    }

    #[test]
    fn convert_gemini_canvas_asset_preserves_inline_body_bytes() {
        let asset = make_browser_media_asset();

        let converted = convert_gemini_canvas_asset(&asset);
        assert_eq!(converted.body_base64.as_deref(), Some("aGVsbG8="));
        assert_eq!(converted.download_token, None);
        assert_eq!(converted.mime_type, "image/jpeg");
        assert_eq!(converted.url, "blob:https://gemini.google.com/example");
    }

    #[test]
    fn infer_gemini_canvas_media_mime_type_prefers_audio_hints_before_url_extension() {
        let resolved = infer_gemini_canvas_media_mime_type(
            None,
            Some("audio/wav"),
            "https://example.com/video.mp4",
            Some("video"),
        );
        assert_eq!(resolved, "audio/wav");
    }

    #[test]
    fn infer_gemini_canvas_media_kind_falls_back_to_audio_for_mp3_urls() {
        let resolved = infer_gemini_canvas_media_kind(
            "application/octet-stream",
            None,
            "https://example.com/path/track.mp3?dl=1",
        );
        assert_eq!(resolved, "audio");
    }

    #[test]
    fn gemini_canvas_asset_url_is_caller_usable_rejects_blob_urls() {
        assert!(gemini_canvas_asset_url_is_caller_usable(
            "https://example.com/image.png"
        ));
        assert!(gemini_canvas_asset_url_is_caller_usable(
            "data:image/png;base64,aaaa"
        ));
        assert!(!gemini_canvas_asset_url_is_caller_usable(
            "blob:https://gemini.google.com/example"
        ));
        assert!(!gemini_canvas_asset_url_is_caller_usable(""));
    }

    #[test]
    fn should_forward_gemini_canvas_download_cookies_accepts_googleusercontent_hops() {
        assert!(should_forward_gemini_canvas_download_cookies(
            "https://gemini.google.com/share/example"
        ));
        assert!(should_forward_gemini_canvas_download_cookies(
            "https://lh3.googleusercontent.com/gg-dl/example"
        ));
        assert!(should_forward_gemini_canvas_download_cookies(
            "https://work.fife.usercontent.google.com/rd-gg-dl/example"
        ));
        assert!(!should_forward_gemini_canvas_download_cookies(
            "https://example.com/image.png"
        ));
    }

    #[test]
    fn normalize_gemini_canvas_direct_http_asset_url_expands_relative_candidates() {
        assert_eq!(
            normalize_gemini_canvas_direct_http_asset_url(
                Some("https://lh3.googleusercontent.com/gg-dl/base"),
                "/rd-gg-dl/example?s=512"
            )
            .as_deref(),
            Some("https://lh3.googleusercontent.com/rd-gg-dl/example?s=512")
        );
        assert_eq!(
            normalize_gemini_canvas_direct_http_asset_url(
                Some("https://gemini.google.com/"),
                "//work.fife.usercontent.google.com/rd-gg-dl/example?s=512"
            )
            .as_deref(),
            Some("https://work.fife.usercontent.google.com/rd-gg-dl/example?s=512")
        );
        assert_eq!(
            normalize_gemini_canvas_direct_http_asset_url(
                Some("https://gemini.google.com/"),
                "lh3.googleusercontent.com/gg-dl/example?s=512"
            )
            .as_deref(),
            Some("https://lh3.googleusercontent.com/gg-dl/example?s=512")
        );
    }

    #[test]
    fn sniff_image_mime_type_from_bytes_detects_known_signatures() {
        assert_eq!(
            sniff_image_mime_type_from_bytes(&[
                0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00,
            ]),
            Some("image/png")
        );
        assert_eq!(
            sniff_image_mime_type_from_bytes(&[0xFF, 0xD8, 0xFF, 0xEE]),
            Some("image/jpeg")
        );
        assert_eq!(
            sniff_image_mime_type_from_bytes(b"RIFFxxxxWEBPpayload"),
            Some("image/webp")
        );
        assert_eq!(
            sniff_image_mime_type_from_bytes(b"GIF89a-trailer"),
            Some("image/gif")
        );
    }

    #[test]
    fn sniff_image_mime_type_from_bytes_rejects_unknown_bytes() {
        assert_eq!(sniff_image_mime_type_from_bytes(b""), None);
        assert_eq!(sniff_image_mime_type_from_bytes(b"plain text"), None);
    }

    #[test]
    fn extract_gemini_canvas_direct_http_image_handoff_url_recovers_relative_hops() {
        assert_eq!(
            extract_gemini_canvas_direct_http_image_handoff_url(
                r#"<a href=\"//work.fife.usercontent.google.com/rd-gg-dl/example?s=512\">here</a>"#
            )
            .as_deref(),
            Some("//work.fife.usercontent.google.com/rd-gg-dl/example?s=512")
        );
        assert_eq!(
            extract_gemini_canvas_direct_http_image_handoff_url(r#"/rd-gg-dl/example?s=512"#)
                .as_deref(),
            Some("/rd-gg-dl/example?s=512")
        );
    }
}
