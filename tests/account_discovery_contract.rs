//! Discovery is explicit, revision-fenced and persisted independently of inference.
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{
    console::secrets::redact_route_document,
    http::router::build_router,
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

#[test]
fn persisted_protocol_selects_the_same_upstream_for_every_text_ingress() {
    use neuro_gateway::{
        protocol::{canonical::EndpointKind, responses::normalize_responses},
        provider_discovery::{binding, CredentialDiscovery, DiscoveredProtocol},
        routing::protocol_resolution::finalize_candidate_protocol_family,
    };
    for protocol in [
        DiscoveredProtocol::ChatCompletions,
        DiscoveredProtocol::Responses,
        DiscoveredProtocol::Messages,
    ] {
        let discovery = CredentialDiscovery {
            probes: Vec::new(),
            protocols: Vec::new(),
            source_url: "https://fixture.test".into(),
            api_base: "https://fixture.test/v1".into(),
            protocol,
            models: vec!["actual".into()],
            verified_models: vec!["actual".into()],
            checked_at: "2026-10-08T00:00:00Z".into(),
            binding: binding("https://fixture.test", "fixture-secret"),
        };
        let config: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
            "id":"pool","adapter":"openai_compatible","base_url":discovery.source_url,
            "model_map":{"alias":"actual","actual":"missing"},
            "credentials":[{"id":"account","api_key":"fixture-secret","discovery":discovery}]}],"model_routes":[]})).unwrap();
        let store = RouteConfigStore::from_document(config).unwrap();
        assert!(
            store.resolve_candidates(Some("actual")).is_empty(),
            "A mapping must not bypass the discovered catalogue"
        );
        for kind in [
            EndpointKind::ChatCompletions,
            EndpointKind::Responses,
            EndpointKind::Messages,
            EndpointKind::Completions,
        ] {
            let mut candidate = store.resolve_candidates(Some("alias")).remove(0);
            let mut request =
                normalize_responses(json!({"model":"alias","input":"hello"})).unwrap();
            request.endpoint_kind = kind;
            assert!(finalize_candidate_protocol_family(&mut candidate, &request).is_some());
            assert_eq!(candidate.protocol_family, discovery.family());
            assert_eq!(candidate.upstream_model.as_deref(), Some("actual"));
        }
    }
}

#[tokio::test]
async fn account_discovery_commit_reload_bridge_and_refresh_failure() {
    let model_reads = Arc::new(AtomicUsize::new(0));
    let generation_calls = Arc::new(AtomicUsize::new(0));
    let reads = model_reads.clone();
    let calls = generation_calls.clone();
    let upstream = Router::new()
        .route("/v1/models", get(move || { let reads = reads.clone(); async move {
            reads.fetch_add(1, Ordering::SeqCst);
            Json(json!({"data":[{"id":"fixture"}]}))
        }}))
        .route("/v1/chat/completions", post(move |Json(body): Json<Value>| {
            let calls = calls.clone(); async move {
                calls.fetch_add(1, Ordering::SeqCst);
                assert_eq!(body["model"], "fixture");
                assert!(body["messages"].is_array());
                Json(json!({"id":"chatcmpl-fixture","object":"chat.completion","created":1,"model":"fixture",
                    "choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
                    "usage":{"prompt_tokens":4,"completion_tokens":1,"total_tokens":5}}))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"pool","adapter":"openai_compatible","base_url":root,
        "credentials":[{"id":"account","api_key":"fixture-secret"}]}],"model_routes":[]}))
    .unwrap();
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let endpoint = "/v1/internal/gateway/console/account-discovery";
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account"}),
        None,
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(model_reads.load(Ordering::SeqCst), 0);
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let discovered = parse_json(response).await;
    assert!(!discovered.to_string().contains("fixture-secret"));
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].credentials[0].discovery =
        Some(serde_json::from_value(discovered["discovery"].clone()).unwrap());
    draft.providers[0].credentials[0].supported_models = vec!["fixture".into()];
    draft.providers[0].supported_models = vec!["fixture".into()];
    let commit = send_console_json(
        &fixture.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        json!({"expectedRevision":discovered["revision"],"document":draft,
            "secretPatches":[{"path":"/providers/0/credentials/0/api_key","operation":"keep"}]}),
        Some(&grant),
        None,
    )
    .await;
    let status = commit.status();
    let body = parse_json(commit).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut persisted: RouteConfigYaml =
        serde_json::from_value(body["routeConfig"]["document"].clone()).unwrap();
    persisted.providers[0].credentials[0].api_key = Some("fixture-secret".into());
    let reloaded = RouteConfigStore::from_document(
        serde_yaml::from_str(&serde_yaml::to_string(&persisted).unwrap()).unwrap(),
    )
    .unwrap();
    assert!(reloaded.resolve_candidates(Some("missing")).is_empty());
    let candidate = reloaded.resolve_candidates(Some("fixture")).remove(0);
    assert_eq!(candidate.payload.base_url, format!("{root}/v1"));
    assert_eq!(
        candidate.payload.chat_completions_path.as_deref(),
        Some("/chat/completions")
    );
    for (path, input) in [
        (
            "/v1/chat/completions",
            json!({"model":"fixture","messages":[{"role":"user","content":"chat"}]}),
        ),
        (
            "/v1/responses",
            json!({"model":"fixture","input":"responses"}),
        ),
        (
            "/v1/messages",
            json!({"model":"fixture","max_tokens":32,"messages":[{"role":"user","content":"messages"}]}),
        ),
    ] {
        let response = build_router(fixture.state.clone())
            .oneshot(
                Request::post(path)
                    .header("authorization", "Bearer public-api-contract-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(input.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert!(body.to_string().contains("OK"), "{path}: {body}");
    }
    assert_eq!(
        model_reads.load(Ordering::SeqCst),
        1,
        "Inference must not rediscover models"
    );
    assert_eq!(generation_calls.load(Ordering::SeqCst), 4);
    let forbidden_override = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account","baseUrl":"https://elsewhere.test"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(forbidden_override.status(), StatusCode::BAD_REQUEST);
    server.abort();
    let _ = server.await;
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let failure = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(failure.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    assert_eq!(
        fixture
            .state
            .route_config
            .resolve_candidates(Some("fixture"))[0]
            .payload
            .base_url,
        format!("{root}/v1")
    );
}
