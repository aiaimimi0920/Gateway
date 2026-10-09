use super::*;
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

#[test]
fn alias_candidates_are_bounded_unique_and_preserve_explicit_prefixes() {
    use DiscoveredProtocol as P;
    let source = "https://fixture.test/proxy";
    let gemini = registry::bases(source, P::GeminiGenerateContent).unwrap();
    for suffix in ["/v1beta", "/v1", "/gemini/v1beta", "/gemini/v1", "/gemini"] {
        assert!(gemini.contains(&format!("{source}{suffix}")));
    }
    let messages = registry::bases(source, P::Messages).unwrap();
    for suffix in ["/claude/v1", "/claude", "/anthropic/v1", "/anthropic"] {
        assert!(messages.contains(&format!("{source}{suffix}")));
    }
    for protocol in registry::ALL {
        for source in [
            "https://fixture.test/proxy",
            "https://fixture.test/proxy/v2",
            "https://fixture.test/proxy/gemini/v1beta",
        ] {
            let bases = registry::bases(source, protocol).unwrap();
            assert!(bases.len() <= endpoint_candidates::MAX_BASES);
            let unique = bases.iter().collect::<std::collections::HashSet<_>>();
            assert_eq!(unique.len(), bases.len());
            assert!(bases
                .iter()
                .all(|b| !b.contains("/gemini/gemini") && !b.contains("/v1beta/v1beta")));
            if source.contains("/gemini/") {
                assert!(bases
                    .iter()
                    .all(|b| b.starts_with("https://fixture.test/proxy/gemini")));
            }
        }
    }
}

#[test]
fn inventories_keep_same_origin_and_reject_error_shaped_success() {
    for protocol in registry::ALL {
        let bases = registry::bases("https://fixture.test/proxy/v1", protocol).unwrap();
        assert!(bases
            .iter()
            .all(|b| b.starts_with("https://fixture.test/proxy")));
        assert!(!registry::has_reply(protocol, &json!({"ok":true})));
        assert!(!registry::has_reply(
            protocol,
            &json!({"error":"no","choices":[{"text":"OK"}]})
        ));
    }
    for bad in [
        "file:///tmp/key",
        "https://u:p@fixture.test",
        "https://fixture.test/?key=x",
    ] {
        assert!(transport::checked_url(bad).is_err());
    }
    assert_eq!(
        registry::request(DiscoveredProtocol::GeminiGenerateContent, "a/b c").0,
        "/models/a%2Fb%20c:generateContent"
    );
}

#[test]
fn catalogue_formats_and_interactions_revision_are_recognized() {
    assert_eq!(
        catalogue::parse(&json!({"models":[{"name":"models/a"},{"name":"models/a"},{"name":""}]})),
        ["a"]
    );
    assert!(catalogue::parse(&json!({"error":"no","data":[{"id":"a"}]})).is_empty());
    assert!(registry::has_reply(
        DiscoveredProtocol::GeminiInteractions,
        &json!({"steps":[{"type":"model_output","content":[{"type":"text","text":"OK"}]}]})
    ));
    assert_eq!(
        registry::request(DiscoveredProtocol::GeminiInteractions, "fixture").1["generation_config"]
            ["max_output_tokens"],
        256
    );
}

#[tokio::test]
async fn root_discovery_persists_working_chat_and_binds_key() {
    let router = Router::new()
        .route(
            "/v1/models",
            get(|| async { Json(json!({"data":[{"id":"fixture"},{"id":"catalog-only"}]})) }),
        )
        .route(
            "/v1/chat/completions",
            post(|| async { Json(json!({"choices":[{"message":{"content":"OK"}}]})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let result = discover(&base, "fixture-key").await.unwrap();
    assert_eq!(result.models.len(), 2);
    assert_eq!(result.verified_models, ["fixture"]);
    assert_eq!(result.protocol, DiscoveredProtocol::ChatCompletions);
    assert_eq!(result.api_base, format!("{base}/v1"));
    let saved = serde_json::to_value(&result).unwrap();
    assert!(!saved.to_string().contains("fixture-key"));
    let reloaded: CredentialDiscovery = serde_json::from_value(saved).unwrap();
    let mut payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible","base_url":base,"api_key":"fixture-key"}))
    .unwrap();
    reloaded.apply(&mut payload).unwrap();
    assert_eq!(
        payload.chat_completions_path.as_deref(),
        Some("/chat/completions")
    );
    payload.api_key = "changed-key".into();
    assert!(reloaded.apply(&mut payload).is_err());
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn falls_back_to_responses_or_messages_without_chat() {
    for (path, reply, protocol) in [
        (
            "/v1/responses",
            json!({"output":[{"content":[{"text":"OK"}]}]}),
            DiscoveredProtocol::Responses,
        ),
        (
            "/v1/messages",
            json!({"content":[{"type":"text","text":"OK"}]}),
            DiscoveredProtocol::Messages,
        ),
    ] {
        let router = Router::new()
            .route(
                "/v1/models",
                get(|| async { Json(json!({"data":[{"id":"fixture"}]})) }),
            )
            .route(
                path,
                post(move || {
                    let reply = reply.clone();
                    async move { Json(reply) }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let result = discover(&base, "fixture-key").await.unwrap();
        assert_eq!(result.protocol, protocol);
        task.abort();
        let _ = task.await;
    }
}
