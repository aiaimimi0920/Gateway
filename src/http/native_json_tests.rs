//! The raw extractor retains the public JSON boundary and cannot bypass body limits.
use super::*;
use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{Request as HttpRequest, StatusCode},
    routing::post,
    Router,
};
use tower::ServiceExt;

fn app() -> Router {
    Router::new()
        .route(
            "/",
            post(|body: NativeJson| async move {
                assert_eq!(body.wire.body.as_ref(), b"{ \"tools\": [] }\n");
                assert_eq!(body.value, serde_json::json!({"tools":[]}));
                StatusCode::OK
            }),
        )
        .layer(DefaultBodyLimit::max(64))
}

#[tokio::test]
async fn native_json_preserves_bytes_and_accepts_standard_json_media_types() {
    for media in [
        "application/json",
        "APPLICATION/JSON; charset=utf-8",
        "application/vendor+json",
    ] {
        let response = app()
            .oneshot(
                HttpRequest::post("/")
                    .header("content-type", media)
                    .body(Body::from("{ \"tools\": [] }\n"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn native_json_rejects_ambiguous_invalid_oversized_or_untyped_requests() {
    for (body, mime, status) in [
        (
            "{\"model\":\"a\",\"model\":\"b\"}".to_owned(),
            "application/json",
            StatusCode::BAD_REQUEST,
        ),
        (
            "not JSON".to_owned(),
            "application/json",
            StatusCode::BAD_REQUEST,
        ),
        (
            "x".repeat(65),
            "application/json",
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        ("{}".to_owned(), "text/plain", StatusCode::BAD_REQUEST),
    ] {
        let response = app()
            .oneshot(
                HttpRequest::post("/")
                    .header("content-type", mime)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
}

#[tokio::test]
async fn native_json_retains_optional_codex_metadata_header_order() {
    let request = HttpRequest::post("/")
        .header("x-openai-subagent", "collab_spawn")
        .header("x-openai-internal-codex-residency", "us")
        .header("x-codex-parent-thread-id", "fixture-parent")
        .header("x-codex-routing-hint", "fixture-route")
        .header("x-openai-memgen-request", "true")
        .header("content-type", "application/json")
        .header("cookie", "must-not-forward")
        .header("x-internal-api-key", "must-not-forward")
        .body(Body::from("{}"))
        .unwrap();
    let native = NativeJson::from_request(request, &()).await.unwrap();
    assert_eq!(
        native.wire.header_order,
        [
            "x-openai-subagent",
            "x-openai-internal-codex-residency",
            "x-codex-parent-thread-id",
            "x-codex-routing-hint",
            "x-openai-memgen-request",
            "content-type"
        ]
    );
}
