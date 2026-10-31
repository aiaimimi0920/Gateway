//! A failed /invoke response is not evidence that generation was never accepted.
use super::*;
use axum::{routing::post, Json, Router};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn media_worker_failures_are_not_replayed_or_fallback_candidates() {
    for operation in ["image", "music", "video"] {
        for legacy_arguments in [false, true] {
            let hits = Arc::new(AtomicUsize::new(0));
            let observed = hits.clone();
            let app = Router::new().route("/invoke", post(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async { (axum::http::StatusCode::BAD_GATEWAY, Json(json!({
                    "ok": false, "error": {"code": "gemini_canvas_browser_worker_failed", "message": "fixture generation response lost", "status": 502}
                }))) }
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let http = Client::builder().no_proxy().build().unwrap();
            let result = if legacy_arguments {
                execute_browser_request_with_recovery(
                    &http,
                    Duration::from_secs(2),
                    "fixture",
                    &base,
                    "https://gemini.google.com",
                    "fixture",
                    "fixture-state",
                    None,
                    None,
                    operation,
                    "fixture",
                    "en",
                )
                .await
            } else {
                execute_browser_request_input_with_recovery(
                    &http,
                    Duration::from_secs(2),
                    "fixture",
                    &base,
                    &json!({"operation": operation}),
                    operation,
                )
                .await
            };
            server.abort();
            let _ = server.await;
            let error = result.unwrap_err();
            assert_eq!(hits.load(Ordering::SeqCst), 1);
            assert!(!error.retryable);
            assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
            assert_eq!(
                error.code.as_deref(),
                Some("gemini_canvas_browser_worker_failed")
            );
        }
    }
}
