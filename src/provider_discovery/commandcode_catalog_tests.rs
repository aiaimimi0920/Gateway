//! CommandCode 公开目录只证明路由声明，不制造推理、费用或套餐权益证据。
use super::*;
use axum::{http::HeaderMap, routing::get, Json, Router};
use serde_json::json;

#[tokio::test]
async fn commandcode_catalog_filters_routes_deduplicates_and_omits_identity() {
    let app = Router::new().route(
        "/provider/v1/models",
        get(|headers: HeaderMap| async move {
            assert!(!headers.contains_key("authorization") && !headers.contains_key("x-api-key"));
            assert!(!headers.contains_key("cookie"));
            Json(json!({"data":[
                {"id":"commandcode-live-text","supported_endpoints":["/chat/completions","/responses"]},
                {"id":"messages-only","supported_endpoints":["/messages"]},
                {"id":"responses-only","supported_endpoints":["/responses"]},
                {"id":"decision-only","supported_endpoints":["/systemone"]},
                {"id":"unknown-route","supported_endpoints":["/future"]},
                {"id":"missing-route"},
                {"id":"commandcode-live-text","supported_endpoints":["/chat/completions"]},
                {"id":"future-text","supported_endpoints":["/chat/completions"]}
            ]}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let catalog = crate::protocol::commandcode::test_support::origin(
        origin,
        fetch_catalog(CatalogProvider::CommandCode, "cpk-synthetic"),
    )
    .await
    .unwrap();
    assert_eq!(
        catalog.models,
        [
            "commandcode-live-text",
            "messages-only",
            "responses-only",
            "decision-only",
            "future-text"
        ]
    );
    assert_eq!(catalog.supported_endpoints["messages-only"], ["/messages"]);
    assert_eq!(catalog.supported_endpoints["decision-only"], ["/systemone"]);
    assert_eq!(
        catalog.supported_endpoints["responses-only"],
        ["/responses"]
    );
    assert_eq!(
        catalog.supported_endpoints["commandcode-live-text"],
        ["/chat/completions", "/responses"]
    );
    assert_eq!(
        catalog.supported_endpoints["future-text"],
        ["/chat/completions"]
    );
    let saved = serde_json::to_value(&catalog).unwrap();
    let restored: ModelCatalog = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.supported_endpoints, catalog.supported_endpoints);
    assert_eq!(catalog.source_url, crate::protocol::commandcode::SOURCE);
    catalog
        .validate(
            Some(crate::protocol::commandcode::PRESET),
            crate::protocol::commandcode::BASE,
        )
        .unwrap();
    for (preset, base) in [
        ("openai", crate::protocol::commandcode::BASE),
        (
            "commandcode-api",
            "https://api.commandcode.ai.evil.invalid/provider/v1",
        ),
        ("commandcode-api", "http://api.commandcode.ai/provider/v1"),
    ] {
        assert!(catalog.validate(Some(preset), base).is_err());
    }
    assert!(!serde_json::to_string(&catalog)
        .unwrap()
        .contains("cpk-synthetic"));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn commandcode_catalog_rejects_invalid_empty_media_only_oversized_and_redirect_results() {
    let app = Router::new()
        .route("/empty", get(|| async { Json(json!({"data":[]})) }))
        .route(
            "/invalid",
            get(|| async {
                Json(json!({"data":[{"id":"*","supported_endpoints":["/chat/completions"]}]}))
            }),
        )
        .route(
            "/media",
            get(|| async { Json(json!({"data":[{"id":"commandcode-u-video"}]})) }),
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
            CatalogProvider::CommandCode,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("cpk-synthetic"));
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
