use super::*;
mod session_compat;
use axum::{
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};

#[test]
fn uses_bounded_unique_slugs_not_display_names_or_codex_api_flags() {
    let models = parse(
        &json!({"models":[{"slug":"web-model","name":"Display", "supported_in_api":false},
        {"slug":"web-model"},{"name":"Not an ID"},{"slug":"*"},{"slug":" bad"}]}),
    )
    .unwrap();
    assert_eq!(
        models,
        vec![json!({"slug":"web-model","visibility":null,"supported_in_api":null})]
    );
    for value in [
        json!({}),
        json!({"models":[]}),
        json!({"models":[{"name":"Display"}]}),
        json!({"models":vec![json!({"slug":"x"});2049]}),
    ] {
        assert!(parse(&value).is_err());
    }
    for origin in [
        "http://chatgpt.com",
        "https://chatgpt.com.evil",
        "https://user@chatgpt.com",
        "https://chatgpt.com/backend-api/codex",
    ] {
        assert!(!accepts_origin(origin));
    }
    assert!(accepts_origin("https://chatgpt.com/"));
}

#[tokio::test]
async fn sends_web_material_only_to_the_selected_endpoint_and_rejects_bad_responses() {
    let app = Router::new()
        .route(
            "/",
            get(|| async { ([("content-type", "text/html")], "<html>fixture</html>") }),
        )
        .route(
            "/models",
            get(|headers: HeaderMap| async move {
                assert_eq!(headers["authorization"], "Bearer fixture-token");
                assert_eq!(headers["cookie"], "session=fixture");
                assert_eq!(headers["user-agent"], "FixtureBrowser");
                assert!(headers.get("chatgpt-account-id").is_none());
                Json(json!({"models":[{"slug":"web-model","name":"Display"}]}))
            }),
        )
        .route(
            "/redirect",
            get(|| async { (StatusCode::FOUND, [("location", "/models")], "private body") }),
        )
        .route(
            "/challenge",
            get(|| async {
                (
                    StatusCode::FORBIDDEN,
                    [("content-type", "text/html"), ("cf-mitigated", "challenge")],
                    "private challenge",
                )
            }),
        )
        .route(
            "/denied",
            get(|| async { (StatusCode::FORBIDDEN, "private access denied") }),
        )
        .route(
            "/challenge503",
            get(|| async {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [("cf-mitigated", "challenge")],
                    "private challenge",
                )
            }),
        )
        .route(
            "/expired",
            get(|| async { (StatusCode::UNAUTHORIZED, "private token") }),
        )
        .route(
            "/html",
            get(|| async { ([("content-type", "text/html")], "private body") }),
        )
        .route(
            "/invalid",
            get(|| async { ([("content-type", "application/json")], "private malformed") }),
        )
        .route(
            "/oversize",
            get(|| async {
                (
                    [("content-type", "application/json")],
                    "x".repeat(MAX_BYTES + 1),
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let payload: ProviderAccountPayload = serde_json::from_value(json!({"adapter":"chatgpt_web_reverse_compatible",
        "baseUrl":format!("http://{address}"), "apiKey":"fixture-token", "headers":{"Cookie":"session=fixture",
        "User-Agent":"FixtureBrowser", "Chatgpt-Account-Id":"must-not-send"}})).unwrap();
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    let catalog = fetch_at(&client, &format!("http://{address}/models"), &payload)
        .await
        .unwrap();
    assert_eq!(catalog["models"][0]["slug"], "web-model");
    assert!(catalog.get("clientVersion").is_none());
    for (path, code) in [
        ("redirect", "chatgpt_web_catalog_http"),
        ("challenge", "chatgpt_web_browser_challenge_required"),
        ("denied", "chatgpt_web_catalog_forbidden"),
        ("challenge503", "chatgpt_web_browser_challenge_required"),
        ("expired", "chatgpt_web_session_invalid"),
        ("html", "chatgpt_web_catalog_not_json"),
        ("invalid", "chatgpt_web_catalog_invalid"),
        ("oversize", "chatgpt_web_catalog_too_large"),
    ] {
        let error = fetch_at(&client, &format!("http://{address}/{path}"), &payload)
            .await
            .unwrap_err();
        assert_eq!(error.code.as_deref(), Some(code));
        assert!(!error.message.contains("private"));
        assert!(!error.message.contains("fixture-token"));
        if path == "challenge503" {
            assert!(error.message.contains("HTTP 503"));
            assert!(!error.message.contains("HTTP 403"));
        }
    }
    task.abort();
    let _ = task.await;
}
