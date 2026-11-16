//! River directory reads are bounded metadata, not generative verification.
use super::*;
use axum::{http::HeaderMap, routing::get, Json, Router};

#[tokio::test]
async fn river_catalog_uses_bearer_get_and_preserves_account_bound_metadata() {
    let app = Router::new().route(
        "/v1/models",
        get(|headers: HeaderMap| async move {
            assert_eq!(headers["authorization"], "Bearer rv_synthetic");
            assert!(!headers.contains_key("cookie"));
            Json(serde_json::json!({"data":[{"id":"org/one"},{"id":"org/two"},{"id":"org/one"}]}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    let catalog = fetch_from(
        &client,
        &format!("http://{address}/v1/models"),
        "rv_synthetic",
        CatalogProvider::River,
    )
    .await
    .unwrap();
    assert_eq!(catalog.source_url, RIVER_SOURCE);
    assert_eq!(catalog.models, ["org/one", "org/two"]);
    catalog
        .validate(Some("river-api"), "https://api.river.ai/v1/")
        .unwrap();
    for (preset, base) in [
        ("openai", "https://api.river.ai/v1"),
        ("river-api", "https://api.river.ai.evil.invalid/v1"),
        ("river-api", "https://api.river.ai/v1?key=x"),
        ("river-api", "http://api.river.ai/v1"),
    ] {
        assert!(catalog.validate(Some(preset), base).is_err());
    }
    assert!(!serde_json::to_string(&catalog)
        .unwrap()
        .contains("verified"));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn river_catalog_rejects_empty_invalid_oversized_and_redirect_results() {
    let app = Router::new()
        .route(
            "/empty",
            get(|| async { Json(serde_json::json!({"data":[]})) }),
        )
        .route(
            "/invalid",
            get(|| async { Json(serde_json::json!({"data":[{"id":"*"}]})) }),
        )
        .route("/large", get(|| async { "x".repeat(LIMIT + 1) }))
        .route(
            "/redirect",
            get(|| async {
                (
                    axum::http::StatusCode::TEMPORARY_REDIRECT,
                    [("location", "https://foreign.invalid/models")],
                )
            }),
        )
        .route(
            "/denied",
            get(|| async { (axum::http::StatusCode::UNAUTHORIZED, "rv_synthetic") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    for path in ["empty", "invalid", "large", "redirect", "denied"] {
        let error = fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "rv_synthetic",
            CatalogProvider::River,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("rv_synthetic"));
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
