use super::*;
use axum::{
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};

#[tokio::test]
async fn zenmux_catalog_is_exact_bounded_metadata_not_generation_evidence() {
    let kind = CatalogProvider::ZenMux;
    let key = "directory-fixture-only";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}{}",
        listener.local_addr().unwrap(),
        url::Url::parse(kind.source()).unwrap().path()
    );
    let app = Router::new().route(
        "/api/v1/models",
        get(move |headers: HeaderMap| async move {
            if headers["authorization"] != format!("Bearer {key}").as_str() {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error":key})),
                );
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({"data":[{"id":"org/Model"},{"id":"org/Model"}]})),
            )
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    let catalog = fetch_from(&client, &url, key, kind).await.unwrap();
    assert_eq!(catalog.models, ["org/Model"]);
    catalog
        .validate(Some("zenmux-api"), "https://zenmux.ai/api/v1/")
        .unwrap();
    assert!(catalog
        .validate(Some("openai"), crate::protocol::zenmux::BASE)
        .is_err());
    assert!(catalog
        .validate(Some("zenmux-api"), "https://zenmux.ai.evil.invalid/api/v1")
        .is_err());
    assert!(!fetch_from(&client, &url, "invalid", kind)
        .await
        .unwrap_err()
        .to_string()
        .contains(key));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
