use super::*;

#[test]
fn conversation_page_missing_asset_error_matches_operation_contracts() {
    let image = gemini_canvas_conversation_page_missing_asset_error(
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        GeminiCanvasPageTargetMode::Resolved,
        "/app/image",
        5,
        30,
        "poll=timeout",
        "<page>",
    );
    assert_eq!(image.http_status, Some(500));
    assert_eq!(
        image.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        image.code.as_deref(),
        Some("gemini_canvas_page_missing_image_asset")
    );
    assert_eq!(
        image.message.as_str(),
        "Gemini Canvas conversation page poll did not expose a usable media asset. locator_mode=resolved; app_path=/app/image; attempts=5; poll_budget_secs=30; failures=poll=timeout; last_page_preview=<page>"
    );

    let music = gemini_canvas_conversation_page_missing_asset_error(
        gemini_canvas::GeminiCanvasMediaOperation::Music,
        GeminiCanvasPageTargetMode::RootAppFallback,
        "/app/music",
        2,
        45,
        "none",
        "<none>",
    );
    assert_eq!(
        music.code.as_deref(),
        Some("gemini_canvas_page_missing_music_asset")
    );

    let video = gemini_canvas_conversation_page_missing_asset_error(
        gemini_canvas::GeminiCanvasMediaOperation::Video,
        GeminiCanvasPageTargetMode::ForcedRootApp,
        "/app/video",
        1,
        60,
        "upstream=empty",
        "<tail>",
    );
    assert_eq!(
        video.code.as_deref(),
        Some("gemini_canvas_page_missing_video_asset")
    );
}

#[test]
fn conversation_page_poll_stage_entry_preserves_contract() {
    let entry = gemini_canvas_conversation_page_poll_stage_entry(3, "page_refresh", "preview-body");

    assert_eq!(entry, "attempt=3 page_refresh=preview-body");
}

#[test]
fn conversation_page_poll_refresh_body_entry_preserves_preview_contract() {
    let body = "refresh body with locator ".repeat(12);
    let entry = gemini_canvas_conversation_page_poll_refresh_body_entry(5, &body);

    assert_eq!(
        entry,
        format!(
            "attempt=5 page_refresh={}",
            compact_response_preview(&body, 220)
        )
    );
}

#[test]
fn conversation_page_poll_refresh_error_entry_preserves_error_summary_contract() {
    let error = GatewayError::service_unavailable("refresh failed")
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_refresh_failed");
    let entry = gemini_canvas_conversation_page_poll_refresh_error_entry(6, &error);

    assert_eq!(
        entry,
        "attempt=6 page_refresh=status=503, code=gemini_canvas_refresh_failed, message=refresh failed"
    );
}

#[test]
fn conversation_page_poll_url_entry_preserves_contract() {
    let entry = gemini_canvas_conversation_page_poll_url_entry(
        4,
        "https://gemini.google.com/app/abc",
        "extract",
        "missing image asset",
    );

    assert_eq!(
        entry,
        "attempt=4 url=https://gemini.google.com/app/abc extract=missing image asset"
    );
}

#[test]
fn conversation_page_poll_fetch_error_entry_preserves_error_summary_contract() {
    let error = GatewayError::service_unavailable("page fetch failed")
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_page_fetch_failed");
    let entry = gemini_canvas_conversation_page_poll_fetch_error_entry(
        8,
        "https://gemini.google.com/app/abc",
        &error,
    );

    assert_eq!(
        entry,
        "attempt=8 url=https://gemini.google.com/app/abc fetch=status=503, code=gemini_canvas_page_fetch_failed, message=page fetch failed"
    );
}

#[test]
fn conversation_page_poll_body_extract_preserves_success_body_contract() {
    let page_body = r#"
            <img src="https:\/\/lh3.googleusercontent.com\/gg-dl\/ABCDEF12345\/image.png">
        "#
    .to_string();

    let (assets, returned_body) =
        try_extract_gemini_canvas_conversation_page_poll_assets_from_body(
            5,
            "https://gemini.google.com/app/abc",
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            page_body.clone(),
        )
        .expect("image asset should be recovered from conversation page body");

    assert_eq!(returned_body, page_body);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(
        assets[0].url,
        "https://lh3.googleusercontent.com/gg-dl/ABCDEF12345/image.png"
    );
}

#[test]
fn conversation_page_poll_body_extract_failure_records_preview_and_entry_contract() {
    let page_body = "pending conversation page without usable asset ".repeat(8);
    let failure = try_extract_gemini_canvas_conversation_page_poll_assets_from_body(
        7,
        "https://gemini.google.com/app/abc",
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        page_body.clone(),
    )
    .expect_err("missing page asset should return extraction failure");

    assert_eq!(
        failure.page_preview,
        compact_response_preview(&page_body, 220)
    );
    assert_eq!(
        failure.failure_entry,
        "attempt=7 url=https://gemini.google.com/app/abc extract=status=500, code=gemini_canvas_page_missing_image_asset, message=Gemini Canvas page blob did not include a usable media asset.; download_candidate_count=0; candidate_filenames=<none>"
    );
}

#[test]
fn conversation_page_poll_timing_preserves_image_contracts() {
    let concrete_page = plan_gemini_canvas_conversation_page_poll_timing(
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        true,
        false,
        Duration::from_secs(300),
    );
    assert_eq!(concrete_page.poll_budget, Duration::from_secs(45));
    assert_eq!(concrete_page.sleep_between_attempts, Duration::from_secs(3));
    assert_eq!(concrete_page.max_fetch_timeout, Duration::from_secs(18));

    let forced_root = plan_gemini_canvas_conversation_page_poll_timing(
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        false,
        true,
        Duration::from_secs(300),
    );
    assert_eq!(forced_root.poll_budget, Duration::from_secs(30));
    assert_eq!(forced_root.sleep_between_attempts, Duration::from_secs(3));
    assert_eq!(forced_root.max_fetch_timeout, Duration::from_secs(10));

    let default_image = plan_gemini_canvas_conversation_page_poll_timing(
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        false,
        false,
        Duration::from_secs(300),
    );
    assert_eq!(default_image.poll_budget, Duration::from_secs(12));
    assert_eq!(default_image.sleep_between_attempts, Duration::from_secs(2));
    assert_eq!(default_image.max_fetch_timeout, Duration::from_secs(12));

    let minimum_budget = plan_gemini_canvas_conversation_page_poll_timing(
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        true,
        false,
        Duration::from_secs(10),
    );
    assert_eq!(minimum_budget.poll_budget, Duration::from_secs(24));
}

#[test]
fn conversation_page_poll_timing_preserves_media_contracts() {
    let music = plan_gemini_canvas_conversation_page_poll_timing(
        gemini_canvas::GeminiCanvasMediaOperation::Music,
        false,
        false,
        Duration::from_secs(300),
    );
    assert_eq!(music.poll_budget, Duration::from_secs(90));
    assert_eq!(music.sleep_between_attempts, Duration::from_secs(4));
    assert_eq!(music.max_fetch_timeout, Duration::from_secs(20));

    let video = plan_gemini_canvas_conversation_page_poll_timing(
        gemini_canvas::GeminiCanvasMediaOperation::Video,
        false,
        false,
        Duration::from_secs(300),
    );
    assert_eq!(video.poll_budget, Duration::from_secs(120));
    assert_eq!(video.sleep_between_attempts, Duration::from_secs(4));
    assert_eq!(video.max_fetch_timeout, Duration::from_secs(20));

    let minimum_media = plan_gemini_canvas_conversation_page_poll_timing(
        gemini_canvas::GeminiCanvasMediaOperation::Music,
        true,
        true,
        Duration::from_secs(10),
    );
    assert_eq!(minimum_media.poll_budget, Duration::from_secs(45));
}

#[test]
fn conversation_page_poll_remaining_budget_enforces_one_second_guard() {
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_remaining(
            Duration::from_secs(10),
            Duration::from_secs(8),
        ),
        Some(Duration::from_secs(2))
    );
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_remaining(
            Duration::from_secs(10),
            Duration::from_secs(9),
        ),
        None
    );
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_remaining(
            Duration::from_secs(10),
            Duration::from_secs(11),
        ),
        None
    );
}

#[test]
fn conversation_page_poll_sleep_decision_preserves_retry_window_guard() {
    assert!(
        should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
            Duration::from_secs(10),
            Duration::from_secs(4),
            Duration::from_secs(4),
        )
    );
    assert!(
        !should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
            Duration::from_secs(10),
            Duration::from_secs(5),
            Duration::from_secs(4),
        )
    );
    assert!(
        !should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
            Duration::from_secs(10),
            Duration::from_secs(11),
            Duration::from_secs(4),
        )
    );
}

#[test]
fn conversation_page_poll_refresh_target_preserves_image_contract() {
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_refresh_target(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            true,
            false,
            Some("https://gemini.google.com/app/conversation"),
            "https://gemini.google.com/app"
        ),
        Some("https://gemini.google.com/app/conversation")
    );
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_refresh_target(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            true,
            false,
            None,
            "https://gemini.google.com/app"
        ),
        Some("https://gemini.google.com/app")
    );
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_refresh_target(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            false,
            true,
            Some("https://gemini.google.com/app/conversation"),
            "https://gemini.google.com/app"
        ),
        Some("https://gemini.google.com/app/conversation")
    );
}

#[test]
fn conversation_page_poll_refresh_target_skips_non_refresh_lanes() {
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_refresh_target(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            false,
            false,
            Some("https://gemini.google.com/app/conversation"),
            "https://gemini.google.com/app"
        ),
        None
    );
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_refresh_target(
            gemini_canvas::GeminiCanvasMediaOperation::Music,
            true,
            true,
            Some("https://gemini.google.com/app/conversation"),
            "https://gemini.google.com/app"
        ),
        None
    );
    assert_eq!(
        resolve_gemini_canvas_conversation_page_poll_refresh_target(
            gemini_canvas::GeminiCanvasMediaOperation::Video,
            true,
            true,
            Some("https://gemini.google.com/app/conversation"),
            "https://gemini.google.com/app"
        ),
        None
    );
}
