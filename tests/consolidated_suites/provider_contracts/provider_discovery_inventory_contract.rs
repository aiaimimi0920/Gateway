//! The onboarding inventory probes every wire, including detection-only protocols.
use axum::{
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{
    provider_discovery::{discover, CredentialDiscovery, DiscoveredProtocol},
    routing::candidate::ProviderAccountPayload,
};
use serde_json::{json, Value};

#[tokio::test]
async fn discovers_twelve_protocols_without_enabling_missing_runtime_bridges() {
    let mut router = Router::new().route(
        "/v1/models",
        get(|| async { Json(json!({"data":[{"id":"fixture"}]})) }),
    );
    let endpoints = [
        (
            "/v1/chat/completions",
            json!({"choices":[{"message":{"content":"OK"}}]}),
        ),
        (
            "/v1/responses",
            json!({"output":[{"content":[{"text":"OK"}]}]}),
        ),
        ("/v1/messages", json!({"content":[{"text":"OK"}]})),
        (
            "/v1beta/models/fixture:generateContent",
            json!({"candidates":[{"content":{"parts":[{"text":"OK"}]}}]}),
        ),
        (
            "/v1beta/interactions",
            json!({"outputs":[{"type":"text","text":"OK"}]}),
        ),
        ("/api/chat", json!({"message":{"content":"OK"},"done":true})),
        ("/api/generate", json!({"response":"OK","done":true})),
        (
            "/v2/chat",
            json!({"message":{"content":[{"type":"text","text":"OK"}]}}),
        ),
        (
            "/model/fixture/converse",
            json!({"output":{"message":{"content":[{"text":"OK"}]}}}),
        ),
        ("/v1/completions", json!({"choices":[{"text":"OK"}]})),
        (
            "/api/v1/services/aigc/text-generation/generation",
            json!({"output":{"text":"OK"}}),
        ),
        (
            "/api/v1/services/aigc/multimodal-generation/generation",
            json!({"output":{"choices":[{"message":{"content":[{"text":"OK"}]}}]}}),
        ),
    ];
    for (path, body) in endpoints {
        router = router.route(
            path,
            post(move || {
                let body = body.clone();
                async move { Json(body) }
            }),
        );
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let result = discover(&root, "fixture-key").await;
    task.abort();
    let _ = task.await;
    let result = result.unwrap();
    assert_eq!(result.protocols.len(), 12);
    assert_eq!(result.probes.len(), 12);
    assert!(result
        .probes
        .iter()
        .all(|p| p.status == "supported" && p.attempts.last().unwrap().status == "supported"));
    assert!(result.probes.iter().filter(|p| p.routing_ready).count() == 5);
    let restored: CredentialDiscovery =
        serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
    let mut payload: ProviderAccountPayload = serde_json::from_value(
        json!({"adapter":"openai_compatible","base_url":root,"api_key":"fixture-key"}),
    )
    .unwrap();
    restored.apply(&mut payload).unwrap();
    assert_eq!(payload.discovered_protocols.len(), 12);
    assert_eq!(payload.adapter, "openai_compatible");
}

#[tokio::test]
async fn native_only_catalogues_work_and_ambiguous_success_is_not_support() {
    for (catalogue_path, catalogue, generation_path, generation, protocol) in [
        (
            "/compatible-mode/v1/models",
            json!({"data":[{"id":"fixture"}]}),
            "/api/v1/services/aigc/text-generation/generation",
            json!({"output":{"text":"OK"}}),
            DiscoveredProtocol::DashscopeText,
        ),
        (
            "/v1beta/models",
            json!({"models":[{"name":"models/fixture","supportedGenerationMethods":["generateContent"]}]}),
            "/v1beta/models/fixture:generateContent",
            json!({"candidates":[{"content":{"parts":[{"text":"OK"}]}}]}),
            DiscoveredProtocol::GeminiGenerateContent,
        ),
        (
            "/api/tags",
            json!({"models":[{"name":"fixture"}]}),
            "/api/chat",
            json!({"message":{"content":"OK"},"done":true}),
            DiscoveredProtocol::OllamaChat,
        ),
        (
            "/v1/models",
            json!({"models":[{"name":"fixture"}]}),
            "/v2/chat",
            json!({"message":{"content":[{"text":"OK"}]}}),
            DiscoveredProtocol::CohereChat,
        ),
        (
            "/foundation-models",
            json!({"modelSummaries":[{"modelId":"fixture"}]}),
            "/model/fixture/converse",
            json!({"output":{"message":{"content":[{"text":"OK"}]}}}),
            DiscoveredProtocol::BedrockConverse,
        ),
    ] {
        let router = Router::new()
            .route(
                catalogue_path,
                get(move || {
                    let body = catalogue.clone();
                    async move { Json(body) }
                }),
            )
            .route(
                generation_path,
                post(move || {
                    let body = generation.clone();
                    async move { Json(body) }
                }),
            )
            .fallback(|| async { Json(json!({"ok":true})) });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let result = discover(&root, "fixture-key").await;
        task.abort();
        let _ = task.await;
        let result = result.unwrap();
        assert_eq!(result.models, ["fixture"]);
        assert_eq!(result.protocols.len(), 1);
        assert_eq!(result.protocol, protocol);
        assert_eq!(
            result
                .probes
                .iter()
                .filter(|p| p.status == "unconfirmed")
                .count(),
            11
        );
        assert!(result
            .probes
            .iter()
            .filter(|p| p.protocol != protocol)
            .flat_map(|p| &p.attempts)
            .all(|a| a.status == "invalid_response"));
        let mut payload: ProviderAccountPayload = serde_json::from_value(
            json!({"adapter":"openai_compatible","base_url":root,"api_key":"fixture-key"}),
        )
        .unwrap();
        result.apply(&mut payload).unwrap();
        assert_eq!(
            payload.adapter,
            if protocol == DiscoveredProtocol::DashscopeText {
                "dashscope_compatible"
            } else {
                "discovery_only"
            }
        );
    }
}

#[tokio::test]
#[ignore = "Explicit operator-authorized live probe of the single HY provider; may incur charges"]
async fn live_hy_inventory() {
    let key = std::env::var("GATEWAY_HY_PROBE_KEY").expect("explicit key required");
    let output = std::env::var("GATEWAY_HY_PROBE_OUTPUT").expect("evidence path required");
    let result = discover("https://ai.hybgzs.com", &key)
        .await
        .expect("live discovery failed");
    let mut evidence: Value = serde_json::to_value(&result).unwrap();
    evidence.as_object_mut().unwrap().remove("binding");
    let text = serde_json::to_string_pretty(&evidence).unwrap();
    assert!(!text.contains(&key));
    std::fs::write(output, text).unwrap();
    println!(
        "models={} confirmed_protocols={}",
        result.models.len(),
        result.protocols.len()
    );
    for probe in result.probes {
        println!(
            "{:?}: {} ({} attempts)",
            probe.protocol,
            probe.status,
            probe.attempts.len()
        );
    }
}
