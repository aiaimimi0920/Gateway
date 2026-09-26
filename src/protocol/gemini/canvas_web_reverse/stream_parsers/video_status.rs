//! Recognize video job, pending, and quota markers in escaped page responses.

use std::sync::OnceLock;

use regex::Regex;

use super::wire_frames::strip_xssi_prefix;

pub fn extract_video_generation_job_id(body: &str) -> Option<String> {
    let normalized = normalize_page_blob_media_text(strip_xssi_prefix(body));
    if !normalized.contains("video_gen_chip") {
        return None;
    }

    static VIDEO_JOB_ID_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = VIDEO_JOB_ID_REGEX.get_or_init(|| {
        Regex::new(r#"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"#)
            .expect("video job id regex must compile")
    });
    regex
        .find(&normalized)
        .map(|matched| matched.as_str().to_string())
}

pub fn response_indicates_video_generation_pending(body: &str) -> bool {
    let normalized = normalize_page_blob_media_text(strip_xssi_prefix(body));
    normalized.contains("video_gen_chip")
        || normalized.contains("正在生成视频")
        || normalized.to_ascii_lowercase().contains("generating video")
        || ((body.contains("\"18\":\"") || body.contains("\\\"18\\\":\\\""))
            && (body.contains("\"21\":[") || body.contains("\\\"21\\\":["))
            && (body.contains("\"44\":true") || body.contains("\\\"44\\\":true")))
        || normalized.contains("BardErrorInfo\",[1053]")
        || normalized.contains("BardErrorInfo\", [1053]")
}

pub fn response_indicates_video_generation_quota_reached(body: &str) -> bool {
    let normalized = normalize_page_blob_media_text(strip_xssi_prefix(body));
    let lower = normalized.to_ascii_lowercase();
    normalized.contains("已达到视频生成数量上限")
        || normalized.contains("后方可继续生成视频")
        || lower.contains("video generation quota is currently exhausted")
        || lower.contains("video generation limit has been reached")
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
