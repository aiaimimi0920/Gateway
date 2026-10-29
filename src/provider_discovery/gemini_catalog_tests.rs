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
async fn gemini_catalog_paginates_chat_names_without_generation_and_is_origin_bound() {
    let calls = Arc::new(AtomicUsize::new(0));
    let reads = calls.clone();
    let app = Router::new().route("/models", get(move |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| {
        let calls = reads.clone();
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(headers["x-goog-api-key"], "fixture-key");
            assert!(!headers.contains_key("authorization"));
            assert_eq!(query["pageSize"], "1000");
            Json(if query.contains_key("pageToken") {
                assert_eq!(query["pageToken"], "next +/&?");
                json!({"models":[{"name":"models/gemini-2.5-flash","supportedGenerationMethods":["generateContent"]},{"name":"models/gemini-2.5-pro","supportedGenerationMethods":["generateContent"]}]})
            } else {
                json!({"models":[{"name":"models/gemini-2.5-flash","supportedGenerationMethods":["generate","generateContent"]},
                    {"name":"embed-only","supportedGenerationMethods":["embed"]}, {"name":"models/unknown"}],"nextPageToken":"next +/&?"})
            })
        }
    }).post(|| async { panic!("catalog must never generate"); #[allow(unreachable_code)] StatusCode::BAD_REQUEST }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/models?pageSize=1000",
        listener.local_addr().unwrap()
    );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    let catalog = fetch_from(&client, &url, "fixture-key", CatalogProvider::Gemini)
        .await
        .unwrap();
    assert_eq!(catalog.models, ["gemini-2.5-flash", "gemini-2.5-pro"]);
    assert_eq!(catalog.source_url, GEMINI_SOURCE);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    catalog
        .validate(
            Some("gemini-api"),
            "https://generativelanguage.googleapis.com/v1beta/",
        )
        .unwrap();
    for base in [
        "http://generativelanguage.googleapis.com",
        "https://generativelanguage.googleapis.com/v2",
        "https://generativelanguage.googleapis.com.evil.invalid",
        "https://generativelanguage.googleapis.com?key=x",
    ] {
        assert!(catalog.validate(Some("gemini-api"), base).is_err());
    }
    assert!(catalog
        .validate(Some("openai"), "https://generativelanguage.googleapis.com")
        .is_err());
    assert!(!serde_json::to_string(&catalog)
        .unwrap()
        .contains("verified"));
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn gemini_catalog_rejects_cycles_redirects_malformed_empty_and_oversized_pages() {
    let app = Router::new()
        .route(
            "/cycle",
            get(|| async { Json(json!({"models":[],"nextPageToken":"same"})) }),
        )
        .route(
            "/redirect",
            get(|| async { (StatusCode::FOUND, [("location", "/cycle")]) }),
        )
        .route(
            "/bad",
            get(|| async { Json(json!({"models":[{"name":"*","supportedGenerationMethods":["generateContent"]}]})) }),
        )
        .route("/empty", get(|| async { Json(json!({"models":[]})) }))
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
    for path in ["cycle", "redirect", "bad", "empty", "huge", "denied"] {
        let error = fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "fixture-key",
            CatalogProvider::Gemini,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("fixture-secret"));
    }
    task.abort();
    let _ = task.await;
}
