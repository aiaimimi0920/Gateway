use super::*;
use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

#[tokio::test]
async fn anthropic_catalog_uses_native_headers_and_bounded_cursor_pages() {
    let calls = Arc::new(AtomicUsize::new(0));
    let reads = calls.clone();
    let app = Router::new().route("/models", get(move |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| {
        let calls = reads.clone();
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(headers["x-api-key"], "fixture-key");
            assert_eq!(headers["anthropic-version"], "2023-06-01");
            assert!(!headers.contains_key("authorization"));
            assert_eq!(query["limit"], "1000");
            Json(if query.contains_key("after_id") {
                assert_eq!(query["after_id"], "next +/&");
                json!({"data":[{"id":"claude-sonnet-4-6"},{"id":"second"}],"has_more":false})
            } else {
                json!({"data":[{"id":"claude-sonnet-4-6"}],"has_more":true,"last_id":"next +/&"})
            })
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/models?limit=1000",
        listener.local_addr().unwrap()
    );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    let catalog = fetch_from(&client, &url, "fixture-key", CatalogProvider::Anthropic)
        .await
        .unwrap();
    assert_eq!(catalog.models, ["claude-sonnet-4-6", "second"]);
    assert_eq!(catalog.source_url, ANTHROPIC_SOURCE);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    catalog
        .validate(Some("anthropic"), "https://api.anthropic.com/")
        .unwrap();
    for base in [
        "http://api.anthropic.com",
        "https://api.anthropic.com/v1",
        "https://api.anthropic.com.evil.invalid",
        "https://api.anthropic.com?key=x",
    ] {
        assert!(catalog.validate(Some("anthropic"), base).is_err());
    }
    assert!(catalog
        .validate(Some("openai"), "https://api.anthropic.com")
        .is_err());
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn anthropic_catalog_rejects_cycles_redirects_and_invalid_pages_without_secret_echo() {
    let app = Router::new()
        .route(
            "/cycle",
            get(|| async {
                Json(json!({"data":[{"id":"same"}],"has_more":true,"last_id":"same"}))
            }),
        )
        .route(
            "/redirect",
            get(|| async { (StatusCode::FOUND, [("location", "/cycle")]) }),
        )
        .route(
            "/bad",
            get(|| async { Json(json!({"data":[{"id":"*"}],"has_more":false})) }),
        )
        .route(
            "/missing",
            get(|| async { Json(json!({"data":[{"id":"valid"}]})) }),
        )
        .route(
            "/empty",
            get(|| async { Json(json!({"data":[],"has_more":false})) }),
        )
        .route("/huge", get(|| async { "x".repeat(LIMIT + 1) }))
        .route(
            "/denied",
            get(|| async { (StatusCode::UNAUTHORIZED, "fixture-secret-error") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    for path in [
        "cycle", "redirect", "bad", "missing", "empty", "huge", "denied",
    ] {
        let error = fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "fixture-key",
            CatalogProvider::Anthropic,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("fixture-secret"));
    }
    task.abort();
    let _ = task.await;
}
