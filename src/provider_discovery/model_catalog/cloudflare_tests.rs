use super::*;
use axum::{extract::Query, http::HeaderMap, routing::get, Json, Router};
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn cloudflare_catalog_paginates_official_text_models_with_exact_token() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/models", get(|headers: HeaderMap, Query(query): Query<HashMap<String, String>>| async move {
        assert_eq!(headers["authorization"], "Bearer synthetic-cf-token");
        assert!(!headers.contains_key("cookie"));
        assert_eq!(query["task"], "Text Generation");
        assert_eq!(query["per_page"], "100");
        let page: u64 = query["page"].parse().unwrap();
        assert!(page <= 2);
        Json(json!({"success":true,"result":[{"name":format!("@cf/org/model-{page}")}],"result_info":{"page":page,"total_pages":2}}))
    }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let base =
        "https://api.cloudflare.com/client/v4/accounts/0123456789abcdef0123456789abcdef/ai/v1";
    let endpoint = Endpoint::parse(base).unwrap();
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    let catalog = fetch_from(
        &client,
        &endpoint,
        "synthetic-cf-token",
        &format!("http://{address}/models?task=Text%20Generation&per_page=100&page=1"),
    )
    .await
    .unwrap();
    assert_eq!(catalog.models, ["@cf/org/model-1", "@cf/org/model-2"]);
    assert_eq!(catalog.account_base_url.as_deref(), Some(base));
    catalog.validate(Some(cf::PRESET), base).unwrap();
    assert!(catalog
        .validate(
            Some(cf::PRESET),
            &base.replace("0123456789abcdef0123456789abcdef", &"a".repeat(32))
        )
        .is_err());
    assert!(catalog.validate(Some("openai"), base).is_err());
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn cloudflare_catalog_fails_closed_on_vendor_error_bad_ids_empty_or_unbounded_pages() {
    for body in [
        json!({"success":false,"result":[{"name":"@cf/model"}]}),
        json!({"success":true,"result":[]}),
        json!({"success":true,"result":[{"name":"*"}]}),
        json!({"success":true,"result":[{"name":"@cf/model"}],"result_info":{"page":1,"total_pages":1000}}),
        json!({"success":true,"result":[{"name":"@cf/model"}],"result_info":{"page":2,"total_pages":2}}),
        json!({"success":true,"result":[{"name":"@cf/model"}],"padding":"x".repeat(LIMIT)}),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/models",
            get(move || {
                let body = body.clone();
                async move { Json(body) }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = rquest::Client::builder()
            .no_proxy()
            .redirect(rquest::redirect::Policy::none())
            .build()
            .unwrap();
        let endpoint = Endpoint::parse(&format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/ai/v1",
            "a".repeat(32)
        ))
        .unwrap();
        let error = fetch_from(
            &client,
            &endpoint,
            "synthetic-cf-token",
            &format!("http://{address}/models?page=1"),
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("synthetic-cf-token"));
        server.abort();
        assert!(server.await.unwrap_err().is_cancelled());
    }
}
