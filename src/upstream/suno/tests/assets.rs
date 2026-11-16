//! Completed music is not successful until the original anonymous asset has been decoded.
use super::*;

#[tokio::test]
async fn suno_completed_music_rejects_html_instead_of_returning_unread_url() {
    let mut html = Reply::status(200, "<html>private signed-token</html>");
    html.mime = "audio/mpeg";
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("a","complete")]})),
        html,
    ])
    .await;
    let error = crate::upstream::public_assets::within(
        &server.origin,
        UpstreamClient::new(3).execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        ),
    )
    .await
    .expect_err("a URL with an HTML body is not playable music");
    assert_eq!(error.code.as_deref(), Some("suno_asset_type"));
    assert!(!error.retryable && !error.message.contains("private"));
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert_eq!(
        server.paths(),
        ["/api/c/check", "/api/generate/v2-web/", "/track.mp3"]
    );
    assert_eq!(server.calls()[2].method, axum::http::Method::GET);
    server.stop().await;
}

#[tokio::test]
async fn suno_music_asset_http_errors_redirect_mime_and_truncation_never_regenerate() {
    let mut truncated = Reply::audio();
    truncated.bytes.pop();
    let mut wrong_mime = Reply::audio();
    wrong_mime.mime = "text/html";
    for (reply, code) in [
        (Reply::status(307, "private"), "suno_asset_http"),
        (
            Reply::status(403, "private signed-token"),
            "suno_asset_http",
        ),
        (wrong_mime, "suno_asset_type"),
        (truncated, "suno_asset_type"),
    ] {
        let server = Server::start(vec![
            Reply::json(json!({"required":false})),
            Reply::json(json!({"clips":[clip("a","complete")]})),
            reply,
        ])
        .await;
        let client = UpstreamClient::new(3);
        let payload = payload(&server.origin);
        let req = request(json!({}));
        let error = crate::upstream::public_assets::within(
            &server.origin,
            crate::retry::execute_with_retry(
                || client.execute_suno_media(&payload, &req, "chirp-v3-5", None),
                &crate::retry::RetryPolicy::default(),
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some(code));
        assert_eq!(error.http_status, Some(502));
        assert!(!error.retryable && !error.message.contains("private"));
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        assert_eq!(
            server.paths().len(),
            3,
            "must not follow, retry or generate again"
        );
        server.stop().await;
    }
}

#[tokio::test]
async fn suno_music_validates_all_targets_before_first_asset_read() {
    for url in [
        "http://assets.example.test/x",
        "https://127.0.0.1/x",
        "https://[::1]/x",
        "https://user:secret@assets.example.test/x",
        "https://assets.example.test:8443/x",
        "https://assets.example.test/x#secret",
    ] {
        let mut foreign = clip("b", "complete");
        foreign["audio_url"] = json!(url);
        let server = Server::start(vec![
            Reply::json(json!({"required":false})),
            Reply::json(json!({"clips":[clip("a","complete"),foreign]})),
        ])
        .await;
        let error = UpstreamClient::new(3)
            .execute_suno_media(
                &payload(&server.origin),
                &request(json!({})),
                "chirp-v3-5",
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("suno_asset_target"));
        assert!(!error.message.contains(url));
        assert_eq!(server.paths().len(), 2);
        server.stop().await;
    }
}

#[tokio::test]
async fn suno_music_asset_limit_and_cumulative_budget_apply_to_real_reads() {
    let size = Reply::audio().bytes.len();
    let clips = crate::protocol::suno::extract_clips_from_feed(
        &json!({"clips":[clip("a","complete"),clip("b","complete")]}),
    )
    .unwrap();
    for (per, total, reads) in [(size - 1, size * 2, 1), (size, size * 2 - 1, 2)] {
        let server = Server::start(vec![Reply::audio(), Reply::audio()]).await;
        let error = crate::upstream::public_assets::within(
            &server.origin,
            super::super::assets::verify_with_limits(
                &clips,
                tokio::time::Instant::now() + std::time::Duration::from_secs(5),
                per,
                total,
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("suno_asset_limit"));
        assert_eq!(server.paths().len(), reads);
        server.stop().await;
    }
}

#[tokio::test]
async fn suno_music_original_deadline_closes_held_asset_without_resubmitting() {
    let mut audio = Reply::audio();
    audio.hold = true;
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("a","complete")]})),
        audio,
    ])
    .await;
    let error = crate::upstream::public_assets::within(
        &server.origin,
        UpstreamClient::new(1).execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("suno_timeout"));
    server.wait_closed().await;
    assert_eq!(server.paths().len(), 3);
    server.stop().await;
}
