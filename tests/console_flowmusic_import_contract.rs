//! FlowMusic JSON uses normal management authentication, grants and CAS; no browser/network I/O.
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use neuro_gateway::{http::router::build_router, routing::config::RouteConfigYaml};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;
const ENDPOINT: &str = "/v1/internal/gateway/console/gemini-auth-sessions";

fn fixture() -> ConsoleStateFixture {
    static OBJECTS: OnceLock<PathBuf> = OnceLock::new();
    OBJECTS.get_or_init(|| {
        let root = std::env::temp_dir().join(format!("flowmusic-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_DRIVER", "local");
        std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR", &root);
        root
    });
    let document: RouteConfigYaml = serde_json::from_value(json!({"providers":[
        {"id":"flowmusic-pool","preset":"producer","base_url":"https://www.flowmusic.app","explicit_credentials_only":true,"credentials":[]},
        {"id":"foreign-pool","preset":"aistudio-web-reverse","base_url":"https://aistudio.google.com","credentials":[]}
    ],"model_routes":[],"aliases":{}})).unwrap();
    ConsoleStateFixture::new(document, true)
}
fn request(f: &ConsoleStateFixture) -> Value {
    json!({"targetFamily":"flowmusic-web","providerId":"flowmusic-pool","accountLabel":"FlowMusic fixture",
        "expectedRevision":f.state.route_config.snapshot().revision().id(),
        "webSessionJson":json!({"schemaVersion":1,"provider":"flowmusic-web","accessToken":"synthetic.access.token","refreshToken":"synthetic-refresh"}).to_string()})
}

#[test]
fn flowmusic_compiled_template_cannot_inherit_image_or_browser_video_capability() {
    let doc: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"flowmusic-web","preset":"producer",
        "base_url":"https://www.flowmusic.app","supported_models":["producer:standard"],
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"flowmusic_web"},
        "credentials":[{"id":"selected","runtime_state_object_key":"credential-runtime/flowmusic/fixture/http-session.json"}]}]})).unwrap();
    let store = neuro_gateway::routing::config::RouteConfigStore::from_document(doc).unwrap();
    let candidates = store.resolve_candidates(Some("producer:standard"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].supported_protocol_families,
        ["producer_music"]
    );
    for endpoint in [
        neuro_gateway::protocol::canonical::EndpointKind::MusicGenerations,
        neuro_gateway::protocol::canonical::EndpointKind::VideosGenerations,
    ] {
        assert_eq!(
            candidates[0].payload.resolve_execution_mode(endpoint),
            neuro_gateway::routing::candidate::ProviderExecutionMode::DirectHttp
        );
    }
}

#[tokio::test]
async fn flowmusic_import_requires_auth_grant_revision_and_matching_provider() {
    let f = fixture();
    let body = request(&f);
    let response = build_router(Arc::clone(&f.state))
        .oneshot(
            Request::post(ENDPOINT)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response =
        send_console_json(&f.state, Method::POST, ENDPOINT, body.clone(), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&f.state).await;
    for (field, value, expected) in [
        ("expectedRevision", json!("stale"), StatusCode::CONFLICT),
        ("providerId", json!("foreign-pool"), StatusCode::BAD_REQUEST),
        (
            "webSessionJson",
            json!("x".repeat(24577)),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let mut body = body.clone();
        body[field] = value;
        let response =
            send_console_json(&f.state, Method::POST, ENDPOINT, body, Some(&grant), None).await;
        assert_eq!(response.status(), expected);
        assert!(!parse_json(response)
            .await
            .to_string()
            .contains("synthetic.access"));
    }
}

#[tokio::test]
async fn flowmusic_import_returns_private_object_reference_and_no_token_or_browser_state() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let response = send_console_json(
        &f.state,
        Method::POST,
        ENDPOINT,
        request(&f),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = parse_json(response).await;
    assert!(!body.to_string().contains("synthetic.access.token"));
    assert!(!body.to_string().contains("synthetic-refresh"));
    let draft = &body["session"]["generatedDrafts"][0];
    assert_eq!(
        draft["credential"]["supported_models"],
        json!(["producer:standard"])
    );
    assert_eq!(draft["secretEdits"], json!([]));
    let key = draft["credential"]["runtime_state_object_key"]
        .as_str()
        .unwrap();
    let bytes = neuro_gateway::object_storage::gateway_object_storage()
        .unwrap()
        .read_bytes(key)
        .await
        .unwrap();
    let stored: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(stored["accessToken"], "synthetic.access.token");
    assert!(stored.get("origins").is_none());
    assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
}
