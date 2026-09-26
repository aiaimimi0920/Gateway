pub(super) fn infer_image_mime_type(url: &str) -> String {
    infer_mime_type(url, "image/png")
}

pub(super) fn infer_audio_mime_type(url: &str) -> String {
    infer_mime_type(url, "audio/mpeg")
}

pub(super) fn infer_video_mime_type(url: &str) -> String {
    infer_mime_type(url, "video/mp4")
}

pub(super) fn infer_mime_type(url: &str, default_mime_type: &str) -> String {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("data:image/") {
        return lower
            .split_once(';')
            .map(|(mime, _)| mime.to_string())
            .unwrap_or_else(|| default_mime_type.to_string());
    }
    if lower.starts_with("data:audio/") {
        return lower
            .split_once(';')
            .map(|(mime, _)| mime.to_string())
            .unwrap_or_else(|| default_mime_type.to_string());
    }
    if lower.starts_with("data:video/") {
        return lower
            .split_once(';')
            .map(|(mime, _)| mime.to_string())
            .unwrap_or_else(|| default_mime_type.to_string());
    }

    if lower.contains(".png") {
        return "image/png".to_string();
    }
    if lower.contains(".jpg") || lower.contains(".jpeg") {
        return "image/jpeg".to_string();
    }
    if lower.contains(".webp") {
        return "image/webp".to_string();
    }
    if lower.contains(".gif") {
        return "image/gif".to_string();
    }
    if lower.contains(".mp3") {
        return "audio/mpeg".to_string();
    }
    if lower.contains(".wav") {
        return "audio/wav".to_string();
    }
    if lower.contains(".ogg") || lower.contains(".opus") {
        return "audio/ogg".to_string();
    }
    if lower.contains(".mp4") {
        return "video/mp4".to_string();
    }
    if lower.contains(".webm") {
        return "video/webm".to_string();
    }

    default_mime_type.to_string()
}
