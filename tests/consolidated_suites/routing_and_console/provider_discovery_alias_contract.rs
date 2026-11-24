//! Alias directories and generation paths are discovered independently and persist exactly.
use axum::{
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{
    provider_discovery::{discover, CredentialDiscovery, DiscoveredProtocol as P},
    routing::candidate::ProviderAccountPayload,
};
use serde_json::json;

#[tokio::test]
async fn root_catalogue_does_not_suppress_alias_catalogues_or_generation() {
    let router = Router::new()
        .route(
            "/v1/models",
            get(|| async { Json(json!({"data":[{"id":"shared-model"}]})) }),
        )
        .route(
            "/claude/v1/models",
            get(|| async { Json(json!({"data":[{"id":"claude-fixture"}]})) }),
        )
        .route(
            "/gemini/v1beta/models",
            get(|| async { Json(json!({"models":[{"name":"models/gemini-fixture"}]})) }),
        )
        .route(
            "/claude/v1/messages",
            post(|| async { Json(json!({"content":[{"type":"text","text":"OK"}]})) }),
        )
        .route(
            "/gemini/v1/models/gemini-fixture:generateContent",
            post(|| async { Json(json!({"candidates":[{"content":{"parts":[{"text":"OK"}]}}]})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let result = discover(&root, "fixture-key").await;
    task.abort();
    let _ = task.await;
    let result = result.unwrap();
    for model in ["shared-model", "claude-fixture", "gemini-fixture"] {
        assert!(result.models.iter().any(|m| m == model));
    }
    assert_eq!(result.protocols.len(), 2);
    let gemini = result
        .protocols
        .iter()
        .find(|c| c.protocol == P::GeminiGenerateContent)
        .unwrap();
    assert_eq!(gemini.api_base, format!("{root}/gemini/v1"));
    assert_eq!(gemini.verified_models, ["gemini-fixture"]);
    let messages = result
        .protocols
        .iter()
        .find(|c| c.protocol == P::Messages)
        .unwrap();
    assert_eq!(messages.api_base, format!("{root}/claude/v1"));
    assert!(result.probes.iter().all(|p| p.attempts.len() <= 24));
    let restored: CredentialDiscovery =
        serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
    let mut payload: ProviderAccountPayload = serde_json::from_value(
        json!({"adapter":"openai_compatible","base_url":root,"api_key":"fixture-key"}),
    )
    .unwrap();
    restored.apply(&mut payload).unwrap();
    assert_eq!(payload.base_url, format!("{root}/claude/v1"));
    assert_eq!(payload.messages_path.as_deref(), Some("/messages"));
}

#[tokio::test]
async fn explicit_alias_and_version_do_not_get_appended_twice() {
    let router = Router::new()
        .route(
            "/proxy/openai/v1/models",
            get(|| async { Json(json!({"data":[{"id":"fixture"}]})) }),
        )
        .route(
            "/proxy/openai/v1/chat/completions",
            post(|| async { Json(json!({"choices":[{"message":{"content":"OK"}}]})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}/proxy/openai/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let result = discover(&root, "fixture-key").await;
    task.abort();
    let _ = task.await;
    let result = result.unwrap();
    assert_eq!(result.api_base, root);
    for attempt in result.probes.iter().flat_map(|p| &p.attempts) {
        assert!(!attempt.endpoint.contains("/openai/openai"));
        assert!(attempt.endpoint.starts_with(root.trim_end_matches("/v1")));
    }
}
