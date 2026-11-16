//! SenseNova 目录只证明账号可见模型，不制造推理、费用或套餐权益证据。
use super::*;
use axum::{http::HeaderMap, routing::get, Json, Router};
use serde_json::json;

#[tokio::test]
async fn sensenova_catalog_filters_media_deduplicates_and_binds_international_origin() {
    let app = Router::new().route(
        "/v1/models",
        get(|headers: HeaderMap| async move {
            assert_eq!(headers["authorization"], "Bearer cpk-synthetic");
            assert!(!headers.contains_key("cookie"));
            Json(
                json!({"data":[{"id":"sensenova-live-text","output_modalities":["text"]},{"id":"sensenova-u-image"},
            {"id":"sensenova-u-video"},{"id":"sensenova-live-text","output_modalities":["text"]},{"id":"future-text","output_modalities":["text"]}]}),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let catalog = crate::protocol::sensenova::test_support::origin(
        origin,
        fetch_catalog(CatalogProvider::SenseNova, "cpk-synthetic"),
    )
    .await
    .unwrap();
    assert_eq!(catalog.models, ["sensenova-live-text", "future-text"]);
    assert_eq!(catalog.source_url, crate::protocol::sensenova::SOURCE);
    catalog
        .validate(
            Some(crate::protocol::sensenova::PRESET),
            crate::protocol::sensenova::BASE,
        )
        .unwrap();
    for (preset, base) in [
        ("openai", crate::protocol::sensenova::BASE),
        (
            "sensenova-token-plan",
            "https://token.sensenova.cn.evil.invalid/v1",
        ),
        ("sensenova-token-plan", "http://token.sensenova.cn/v1"),
    ] {
        assert!(catalog.validate(Some(preset), base).is_err());
    }
    let mut bad = catalog.clone();
    bad.models.push("sensenova-u-image".into());
    assert!(bad
        .validate(
            Some("sensenova-token-plan"),
            crate::protocol::sensenova::BASE
        )
        .is_err());
    assert!(!serde_json::to_string(&catalog)
        .unwrap()
        .contains("cpk-synthetic"));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn sensenova_catalog_rejects_invalid_empty_media_only_oversized_and_redirect_results() {
    let app = Router::new()
        .route("/empty", get(|| async { Json(json!({"data":[]})) }))
        .route(
            "/invalid",
            get(|| async { Json(json!({"data":[{"id":"*"}]})) }),
        )
        .route(
            "/media",
            get(|| async { Json(json!({"data":[{"id":"sensenova-u-video"}]})) }),
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
            get(|| async { (axum::http::StatusCode::UNAUTHORIZED, "cpk-synthetic") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    for path in ["empty", "invalid", "media", "large", "redirect", "denied"] {
        let error = fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "cpk-synthetic",
            CatalogProvider::SenseNova,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("cpk-synthetic"));
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
