//! Whole direct-executor happy path, partial snapshots, terminal failures and explicit no-wait.
use super::*;
use std::time::Duration;

#[tokio::test]
async fn suno_direct_polls_only_original_ids_and_restores_submission_order() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("a","submitted"),clip("b","submitted")]})),
        Reply::json(json!({"clips":[]})),
        Reply::json(json!({"clips":[clip("a","complete")]})),
        Reply::json(json!({"clips":[clip("b","complete"),clip("a","complete")]})),
        Reply::audio(),
        Reply::audio(),
    ])
    .await;
    let result = crate::upstream::public_assets::within(
        &server.origin,
        UpstreamClient::new(6).execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        ),
    )
    .await
    .unwrap();
    assert_eq!(result["completed"], true);
    assert_eq!(result["data"][0]["id"], "a");
    assert_eq!(result["data"][1]["id"], "b");
    assert!(
        result.get("usage").is_none(),
        "unknown upstream cost must stay unknown"
    );
    let calls = server.calls();
    assert_eq!(calls.len(), 7);
    assert_eq!(calls[0].body, json!({"ctype":"generation"}));
    assert_eq!(calls[1].body["gpt_description_prompt"], "synthetic music");
    for call in &calls[..5] {
        assert_eq!(call.headers["cookie"], "__session=synthetic-session");
        assert_eq!(call.headers["authorization"], "Bearer synthetic-clerk");
        assert_eq!(call.headers["device-id"], "synthetic-device");
        assert_eq!(call.headers["origin"], "https://suno.com");
    }
    for call in &calls[2..5] {
        assert_eq!(call.path, "/api/feed/v3");
        assert_eq!(call.body["filters"]["ids"]["clipIds"], json!(["a", "b"]));
    }
    assert_ne!(
        calls[1].headers["browser-token"],
        calls[4].headers["browser-token"]
    );
    for call in &calls[5..] {
        assert_eq!(call.method, axum::http::Method::GET);
        assert_eq!(call.path, "/track.mp3");
        for name in [
            "cookie",
            "authorization",
            "origin",
            "referer",
            "device-id",
            "browser-token",
        ] {
            assert!(!call.headers.contains_key(name), "asset received {name}");
        }
    }
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_never_completes_from_disjoint_partial_snapshots() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("a","submitted"),clip("b","submitted")]})),
        Reply::json(json!({"clips":[clip("a","complete")]})),
        Reply::json(json!({"clips":[clip("b","complete")]})),
    ])
    .await;
    let started = std::time::Instant::now();
    let error = UpstreamClient::new(3)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("suno_timeout"));
    assert_eq!(error.http_status, Some(504));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(server.paths().len(), 4);
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_explicit_no_wait_preserves_pending_ids_without_claiming_completion() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("a","submitted"),clip("b","queued")]})),
    ])
    .await;
    let result = UpstreamClient::new(3)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({"waitCompletion":false})),
            "chirp-v3-5",
            None,
        )
        .await
        .unwrap();
    assert_eq!(result["completed"], false);
    assert_eq!(result["data"].as_array().unwrap().len(), 2);
    assert_eq!(server.paths().len(), 2);
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_duplicate_and_malformed_clip_sets_are_not_filtered() {
    for clips in [
        json!([]),
        json!([clip("a", "complete"), clip("a", "complete")]),
        json!([clip("a", "complete"), null]),
        json!([{"id":7}]),
        json!([{"id":" a"}]),
        json!([{"id":""}]),
        json!([{"id":"a","status":5}]),
    ] {
        let server = Server::start(vec![
            Reply::json(json!({"required":false})),
            Reply::json(json!({"clips":clips})),
        ])
        .await;
        let error = UpstreamClient::new(3)
            .execute_suno_media(
                &payload(&server.origin),
                &request(json!({"waitCompletion":false})),
                "chirp-v3-5",
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error.code.as_deref(),
            Some("suno_clip_identity" | "suno_clip_status")
        ));
        assert!(!error.retryable);
        assert_eq!(server.paths().len(), 2);
        server.stop().await;
    }
}

#[tokio::test]
async fn suno_direct_failed_or_duplicate_feed_never_hides_behind_a_complete_clip() {
    for clips in [
        json!([clip("a", "complete"), clip("b", "error")]),
        json!([clip("a", "complete"), clip("a", "complete")]),
    ] {
        let server = Server::start(vec![
            Reply::json(json!({"required":false})),
            Reply::json(json!({"clips":[clip("a","submitted"),clip("b","submitted")]})),
            Reply::json(json!({"clips":clips})),
        ])
        .await;
        let error = UpstreamClient::new(4)
            .execute_suno_media(
                &payload(&server.origin),
                &request(json!({})),
                "chirp-v3-5",
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error.code.as_deref(),
            Some("suno_generation_failed" | "suno_clip_identity")
        ));
        assert_eq!(server.paths().len(), 3);
        server.stop().await;
    }
}

#[tokio::test]
async fn suno_direct_challenge_gate_stops_submission_and_does_not_solve_captcha() {
    let server = Server::start(vec![Reply::json(json!({"required":true}))]).await;
    let error = UpstreamClient::new(3)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("suno_challenge_required"));
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert_eq!(server.paths(), ["/api/c/check"]);
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_validates_model_before_any_network_call() {
    let server = Server::start(vec![]).await;
    let error = UpstreamClient::new(3)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "unknown-current-model",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("invalid_suno_model"));
    assert!(server.paths().is_empty());
    server.stop().await;
}
