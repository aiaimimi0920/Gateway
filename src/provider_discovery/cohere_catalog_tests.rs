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
async fn cohere_catalog_paginates_chat_names_without_generation_and_is_origin_bound() {
    let calls = Arc::new(AtomicUsize::new(0));
    let reads = calls.clone();
    let app = Router::new().route("/models", get(move |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| {
        let calls = reads.clone();
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(headers["authorization"], "Bearer fixture-key");
            assert_eq!(query["endpoint"], "chat");
            assert_eq!(query["page_size"], "1000");
            Json(if query.contains_key("page_token") {
                assert_eq!(query["page_token"], "next +/&?");
                json!({"models":[{"name":"command-r7b-12-2024","endpoints":["chat"]},{"name":"command-a","endpoints":["chat"]}]})
            } else {
                json!({"models":[{"name":"command-r7b-12-2024","endpoints":["generate","chat"]},
                    {"name":"embed-only","endpoints":["embed"]}, {"name":"old","endpoints":["chat"],"is_deprecated":true}],"next_page_token":"next +/&?"})
            })
        }
    }).post(|| async { panic!("catalog must never generate"); #[allow(unreachable_code)] StatusCode::BAD_REQUEST }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/models?endpoint=chat&page_size=1000",
        listener.local_addr().unwrap()
    );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    let catalog = fetch_from(&client, &url, "fixture-key", CatalogProvider::Cohere)
        .await
        .unwrap();
    assert_eq!(catalog.models, ["command-r7b-12-2024", "command-a"]);
    assert_eq!(catalog.source_url, COHERE_SOURCE);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    catalog
        .validate(Some("cohere-chat"), "https://api.cohere.com/")
        .unwrap();
    for base in [
        "http://api.cohere.com",
        "https://api.cohere.com/v2",
        "https://api.cohere.com.evil.invalid",
        "https://api.cohere.com?key=x",
    ] {
        assert!(catalog.validate(Some("cohere-chat"), base).is_err());
    }
    assert!(catalog
        .validate(Some("openai"), "https://api.cohere.com")
        .is_err());
    assert!(!serde_json::to_string(&catalog)
        .unwrap()
        .contains("verified"));
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn cohere_catalog_rejects_cycles_redirects_malformed_empty_and_oversized_pages() {
    let app = Router::new()
        .route(
            "/cycle",
            get(|| async { Json(json!({"models":[],"next_page_token":"same"})) }),
        )
        .route(
            "/redirect",
            get(|| async { (StatusCode::FOUND, [("location", "/cycle")]) }),
        )
        .route(
            "/bad",
            get(|| async { Json(json!({"models":[{"name":"*","endpoints":["chat"]}]})) }),
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
            CatalogProvider::Cohere,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("fixture-secret"));
    }
    task.abort();
    let _ = task.await;
}
