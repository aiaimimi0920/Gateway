use super::*;
use axum::{
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn catalog_get_is_bounded_and_never_follows_redirects_or_generates() {
    let posts = Arc::new(AtomicUsize::new(0));
    let posted = posts.clone();
    let app = Router::new()
        .route(
            "/models",
            get(|headers: HeaderMap| async move {
                assert_eq!(headers["authorization"], "Bearer fixture-key");
                assert!(!headers.contains_key("cookie"));
                Json(serde_json::json!({"data":[{"id":"one"},{"id":"two"},{"id":"one"}]}))
            }).post(move || async move { posted.fetch_add(1, Ordering::SeqCst); StatusCode::BAD_REQUEST }),
        )
        .route(
            "/redirect",
            get(|| async { (StatusCode::FOUND, [("location", "/models")]) }),
        )
        .route(
            "/bad",
            get(|| async { Json(serde_json::json!({"data":[{"id":"*"}]})) }),
        )
        .route("/huge", get(|| async { "x".repeat(LIMIT + 1) }))
        .route("/huge-or", get(|| async { "x".repeat(CatalogProvider::OpenRouter.limit() + 1) }))
        .route("/large", get(|| async { Json(serde_json::json!({"data":[{"id":"large-model"}],"padding":"x".repeat(LIMIT + 1)})) }))
        .route(
            "/denied",
            get(|| async { (StatusCode::UNAUTHORIZED, "fixture-secret-upstream-error") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    let catalog = fetch_from(
        &client,
        &format!("http://{address}/models"),
        "fixture-key",
        CatalogProvider::Nvidia,
    )
    .await
    .unwrap();
    assert_eq!(catalog.models, ["one", "two"]);
    catalog
        .validate(Some("nvidia-openai"), NVIDIA_BASE)
        .unwrap();
    let json = serde_json::to_value(&catalog).unwrap();
    assert!(json.get("verifiedModels").is_none());
    for path in ["redirect", "bad", "huge", "denied"] {
        let error = fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "fixture-key",
            CatalogProvider::Nvidia,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("fixture-secret"));
    }
    // Same transport/parser, but source and size budget remain provider-bound.
    let openrouter = fetch_from(
        &client,
        &format!("http://{address}/models"),
        "fixture-key",
        CatalogProvider::OpenRouter,
    )
    .await
    .unwrap();
    assert_eq!(openrouter.models, ["one", "two"]);
    assert_eq!(openrouter.source_url, OPENROUTER_SOURCE);
    openrouter
        .validate(Some("openrouter-openai"), "https://openrouter.ai/api/v1")
        .unwrap();
    assert!(openrouter
        .validate(Some("nvidia-openai"), NVIDIA_BASE)
        .is_err());
    for path in ["redirect", "bad", "denied", "huge-or"] {
        assert!(fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "fixture-key",
            CatalogProvider::OpenRouter
        )
        .await
        .is_err());
    }
    assert!(fetch_from(
        &client,
        &format!("http://{address}/large"),
        "fixture-key",
        CatalogProvider::Nvidia
    )
    .await
    .is_err());
    assert_eq!(
        fetch_from(
            &client,
            &format!("http://{address}/large"),
            "fixture-key",
            CatalogProvider::OpenRouter
        )
        .await
        .unwrap()
        .models,
        ["large-model"]
    );
    assert_eq!(posts.load(Ordering::SeqCst), 0);
    let poe = fetch_from(
        &client,
        &format!("http://{address}/models"),
        "fixture-key",
        CatalogProvider::Poe,
    )
    .await
    .unwrap();
    assert_eq!(poe.source_url, POE_SOURCE);
    assert_eq!(poe.models, ["one", "two"]);
    for path in ["redirect", "bad", "huge", "denied"] {
        assert!(fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "fixture-key",
            CatalogProvider::Poe
        )
        .await
        .is_err());
    }
    assert_eq!(posts.load(Ordering::SeqCst), 0);
    task.abort();
    let _ = task.await;
}

#[test]
fn poe_catalog_origin_and_metadata_are_strictly_bound() {
    let catalog = ModelCatalog {
        source_url: "https://api.poe.com/v1/models".into(),
        checked_at: "2026-10-26T13:00:00Z".into(),
        models: vec!["gpt-4o-mini".into()],
    };
    for base in ["https://api.poe.com", "https://api.poe.com/v1/"] {
        catalog.validate(Some("poe-openai"), base).unwrap();
    }
    for base in [
        "http://api.poe.com/v1",
        "https://api.poe.com.evil.invalid/v1",
        "https://api.poe.com/v1?key=x",
    ] {
        assert!(catalog.validate(Some("poe-openai"), base).is_err());
    }
    assert!(catalog
        .validate(Some("openai"), "https://api.poe.com/v1")
        .is_err());
}

#[tokio::test]
async fn groq_catalog_is_origin_bound_bounded_metadata_without_generation() {
    let posts = Arc::new(AtomicUsize::new(0));
    let posted = posts.clone();
    let app = Router::new()
        .route("/models", get(|headers: HeaderMap| async move {
            assert_eq!(headers["authorization"], "Bearer fixture-key");
            assert!(!headers.contains_key("cookie"));
            Json(serde_json::json!({"data":[{"id":"openai/gpt-oss-20b"},{"id":"another"},{"id":"another"}]}))
        }).post(move || async move { posted.fetch_add(1, Ordering::SeqCst); StatusCode::BAD_REQUEST }))
        .route("/redirect", get(|| async { (StatusCode::FOUND, [("location", "/models")]) }))
        .route("/bad", get(|| async { Json(serde_json::json!({"data":[{"id":"*"}]})) }))
        .route("/empty", get(|| async { Json(serde_json::json!({"data":[]})) }))
        .route("/huge", get(|| async { "x".repeat(LIMIT + 1) }))
        .route("/denied", get(|| async { (StatusCode::UNAUTHORIZED, "fixture-secret-error") }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    let catalog = fetch_from(
        &client,
        &format!("http://{address}/models"),
        "fixture-key",
        CatalogProvider::Groq,
    )
    .await
    .unwrap();
    assert_eq!(catalog.source_url, GROQ_SOURCE);
    assert_eq!(catalog.models, ["openai/gpt-oss-20b", "another"]);
    catalog
        .validate(Some("groq-openai"), "https://api.groq.com/openai/v1/")
        .unwrap();
    assert!(serde_json::to_value(&catalog)
        .unwrap()
        .get("verifiedModels")
        .is_none());
    for base in [
        "http://api.groq.com/openai/v1",
        "https://api.groq.com",
        "https://api.groq.com/openai",
        "https://api.groq.com.evil.invalid/openai/v1",
        "https://api.groq.com/openai/v1?key=x",
    ] {
        assert!(catalog.validate(Some("groq-openai"), base).is_err());
    }
    assert!(catalog
        .validate(Some("openai"), "https://api.groq.com/openai/v1")
        .is_err());
    for path in ["redirect", "bad", "empty", "huge", "denied"] {
        let error = fetch_from(
            &client,
            &format!("http://{address}/{path}"),
            "fixture-key",
            CatalogProvider::Groq,
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("fixture-secret"));
    }
    assert_eq!(posts.load(Ordering::SeqCst), 0);
    task.abort();
    let _ = task.await;
}

#[test]
fn openrouter_catalog_origin_and_metadata_are_strictly_bound() {
    let mut catalog = ModelCatalog {
        source_url: OPENROUTER_SOURCE.into(),
        checked_at: "2026-10-26T04:00:00Z".into(),
        models: vec!["vendor/one".into()],
    };
    for base in [
        "https://openrouter.ai",
        "https://openrouter.ai/api/",
        "https://openrouter.ai/api/v1",
    ] {
        catalog.validate(Some("openrouter-openai"), base).unwrap();
        catalog.validate(Some("openai"), base).unwrap();
    }
    for base in [
        "http://openrouter.ai/api/v1",
        "https://openrouter.ai.evil.invalid/api/v1",
        "https://openrouter.ai/api/v1?key=secret",
    ] {
        assert!(catalog.validate(Some("openrouter-openai"), base).is_err());
    }
    assert!(catalog
        .validate(Some("other"), "https://openrouter.ai/api/v1")
        .is_err());
    catalog.checked_at = "invalid".into();
    assert!(catalog
        .validate(Some("openrouter-openai"), "https://openrouter.ai/api/v1")
        .is_err());
}

#[test]
fn openai_catalog_requires_platform_origin_and_preserves_legacy_openrouter() {
    let catalog = ModelCatalog {
        source_url: OPENAI_SOURCE.into(),
        checked_at: "2026-10-27T04:00:00Z".into(),
        models: vec!["gpt-4o-mini".into()],
    };
    for base in ["https://api.openai.com/v1", "https://api.openai.com/v1/"] {
        catalog.validate(Some("openai"), base).unwrap();
    }
    for base in [
        "http://api.openai.com/v1",
        "https://api.openai.com",
        "https://api.openai.com.evil.invalid/v1",
        "https://api.openai.com/v1?key=x",
        "https://openrouter.ai/api/v1",
    ] {
        assert!(catalog.validate(Some("openai"), base).is_err());
    }
    assert!(catalog
        .validate(
            Some("chatgpt-codex-oauth-official-api"),
            "https://api.openai.com/v1"
        )
        .is_err());
    assert!(CatalogProvider::OpenRouter.accepts(Some("openai"), "https://openrouter.ai/api/v1"));
}

#[test]
fn catalog_metadata_is_optional_validated_and_not_generation_evidence() {
    let old: crate::routing::config::ProviderCredentialYaml = serde_json::from_value(
        serde_json::json!({"id":"old", "api_key":"fixture-key", "supported_models":["manual"]}),
    )
    .unwrap();
    assert!(old.model_catalog.is_none());
    assert!(serde_json::to_value(old)
        .unwrap()
        .get("model_catalog")
        .is_none());
    let mut catalog = ModelCatalog {
        source_url: NVIDIA_SOURCE.into(),
        checked_at: "2026-10-26T04:00:00Z".into(),
        models: vec!["one".into()],
    };
    catalog
        .validate(Some("nvidia-openai"), NVIDIA_BASE)
        .unwrap();
    assert!(catalog.validate(Some("other"), NVIDIA_BASE).is_err());
    assert!(catalog
        .validate(Some("nvidia-openai"), "https://evil.invalid")
        .is_err());
    catalog.models = vec!["one".into(), "one".into()];
    assert!(catalog
        .validate(Some("nvidia-openai"), NVIDIA_BASE)
        .is_err());
}
