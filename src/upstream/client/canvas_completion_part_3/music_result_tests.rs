use super::*;

const WEB: &str = "gemini_web_reverse_modular_compatible";

fn asset(kind: &str, mime_type: &str) -> GeminiCanvasBrowserMediaAsset {
    GeminiCanvasBrowserMediaAsset {
        kind: kind.to_string(),
        url: "https://fixture.invalid/media".to_string(),
        mime_type: mime_type.to_string(),
        body_base64: Some("Zml4dHVyZQ==".to_string()),
        alt: None,
        width: None,
        height: None,
        duration_seconds: None,
    }
}

#[test]
fn ordinary_web_music_never_accepts_cover_video_from_an_old_or_remote_worker() {
    assert!(select_music_asset(WEB, &[asset("video", "video/mp4")]).is_none());
    assert!(select_music_asset(WEB, &[asset("audio", "video/mp4")]).is_none());
    let selected = select_music_asset(
        WEB,
        &[asset("video", "video/mp4"), asset("audio", "audio/mpeg")],
    )
    .expect("the MP3 track is available");
    assert_eq!(selected.kind, "audio");
    assert_eq!(selected.mime_type, "audio/mpeg");
}

#[test]
fn legacy_music_result_selection_is_unchanged() {
    let selected = select_music_asset("gemini_canvas_compatible", &[asset("video", "video/mp4")])
        .expect("legacy video contract remains supported");
    assert_eq!(selected.kind, "video");
}
