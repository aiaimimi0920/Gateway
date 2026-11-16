//! A bound remote generation cannot fall back, redirect or accept an old worker's receipt.
use super::*;
#[path = "../../suno/tests/fixture.rs"]
mod peer;
use peer::{Reply, Server};

fn assert_abort(error: GatewayError) {
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!error.message.contains("private-remote"));
}
fn client(base: String) -> UpstreamClient {
    let mut client = UpstreamClient::new(2);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(base);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    client
}
#[tokio::test]
async fn suno_remote_uncertainty_never_requests_local_replay() {
    let result = client(unused_loopback_base_url())
        .execute_remote_browser_executor("suno", "exact", EndpointKind::MusicGenerations, json!({}))
        .await;
    assert_abort(result.expect_err("an uncertain dispatch must not start a local generation"));
    for (status, body) in [
        (503, "private-remote"),
        (200, "not-json private-remote"),
        (200, r#"{"ok":true}"#),
        (200, r#"{"ok":true,"result":null}"#),
        (
            200,
            r#"{"ok":false,"error":{"status":429,"code":"fixture_rate_limit"}}"#,
        ),
    ] {
        let (base, server) = single_response_executor_base_url(status, "Fixture", body);
        let result = client(base)
            .execute_remote_browser_executor(
                "suno",
                "exact",
                EndpointKind::MusicGenerations,
                json!({}),
            )
            .await;
        server.join().unwrap();
        assert_abort(result.expect_err("remote response cannot decline into local execution"));
    }
}
#[tokio::test]
async fn suno_remote_model_binding_uses_versioned_entry_and_resolved_alias() {
    let server = Server::start(vec![Reply::json(json!({"ok":true,"result":{
        "clips":[{"id":"synthetic","status":"submitted"}],"completed":false,"wireModel":"chirp-auk"}}))]).await;
    let payload = make_payload("suno_compatible", &server.origin);
    let request = crate::protocol::suno::normalize_music_generations(json!({
        "model":"public-alias","prompt":"synthetic fixture","waitCompletion":false}))
    .unwrap();
    let result = client(server.origin.clone())
        .execute_suno_browser_backed("exact", &payload, &request, "chirp-v3-0", None)
        .await
        .unwrap();
    let calls = server.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].path,
        "/v1/internal/browser-executor/suno-bound-v1/execute"
    );
    assert_eq!(calls[0].body["input"]["requestBody"]["model"], "chirp-auk");
    assert_eq!(
        calls[0].body["input"]["requestBody"]["prompt"],
        "synthetic fixture"
    );
    assert_eq!(calls[0].body["input"]["waitCompletion"], false);
    assert_eq!(result["model"], "chirp-v3-0");
    server.stop().await;
}
#[tokio::test]
async fn suno_remote_missing_or_wrong_binding_receipt_is_not_success() {
    for wire in [Value::Null, json!("chirp-auk-turbo")] {
        let server = Server::start(vec![Reply::json(json!({"ok":true,"result":{
            "clips":[{"id":"synthetic","status":"submitted"}],"completed":false,"wireModel":wire}}))]).await;
        let payload = make_payload("suno_compatible", &server.origin);
        let request = crate::protocol::suno::normalize_music_generations(
            json!({"prompt":"synthetic fixture","waitCompletion":false}),
        )
        .unwrap();
        let result = client(server.origin.clone())
            .execute_suno_browser_backed("exact", &payload, &request, "chirp-v3-0", None)
            .await;
        let error =
            result.expect_err("old or mismatched workers cannot label a model-bound success");
        assert_eq!(error.code.as_deref(), Some("suno_browser_model_mismatch"));
        assert_abort(error);
        assert_eq!(server.calls().len(), 1);
        server.stop().await;
    }
}
#[tokio::test]
async fn suno_old_remote_endpoint_and_redirect_do_not_dispatch_legacy_generation() {
    use axum::{routing::post, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    for status in [404, 307, 308] {
        let legacy_calls = Arc::new(AtomicUsize::new(0));
        let legacy = legacy_calls.clone();
        let path = "/v1/internal/browser-executor/execute";
        let app = Router::new()
            .route(
                path,
                post(move || {
                    legacy.fetch_add(1, Ordering::SeqCst);
                    async { axum::Json(json!({"ok":true,"result":{}})) }
                }),
            )
            .route(
                "/v1/internal/browser-executor/suno-bound-v1/execute",
                post(move || async move {
                    (
                        axum::http::StatusCode::from_u16(status).unwrap(),
                        [("location", path)],
                        "private-remote",
                    )
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = client(base)
            .execute_remote_browser_executor(
                "suno",
                "exact",
                EndpointKind::MusicGenerations,
                json!({}),
            )
            .await;
        server.abort();
        let _ = server.await;
        assert_abort(result.expect_err("old endpoint or redirect cannot be replayed locally"));
        assert_eq!(
            legacy_calls.load(Ordering::SeqCst),
            0,
            "never negotiate by sending a paid task to the legacy route"
        );
    }
}

#[tokio::test]
async fn suno_browser_unknown_models_and_unimplemented_options_do_not_dispatch() {
    let server = Server::start(vec![]).await;
    let payload = make_payload("suno_compatible", &server.origin);
    for (model, extra) in [
        ("unknown", json!({})),
        ("chirp-v3-5", json!({"lyrics":"custom"})),
        ("chirp-v3-5", json!({"mv":"conflicting-native"})),
    ] {
        let mut body = json!({"prompt":"synthetic fixture"});
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let request = crate::protocol::suno::normalize_music_generations(body).unwrap();
        assert!(client(server.origin.clone())
            .execute_suno_browser_backed("exact", &payload, &request, model, None)
            .await
            .is_err());
        assert!(server.calls().is_empty());
    }
    server.stop().await;
}
