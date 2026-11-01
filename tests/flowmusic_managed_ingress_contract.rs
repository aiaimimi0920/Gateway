//! Managed material is deliberately invalid, so this normal-stack ingress cannot submit upstream work.
use axum::{body::Body, http::Request};
use neuro_gateway::{http::router::build_router, local_runtime::LocalRuntime};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

#[tokio::test]
async fn malformed_managed_session_fails_closed_on_normal_test_stack() {
    let root = std::env::temp_dir().join(format!("flowmusic-managed-{}", uuid::Uuid::new_v4()));
    let objects = root.join("objects");
    let key = "credential-runtime/flowmusic/synthetic/http-session.json";
    let path = objects.join(key);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        br#"{"schemaVersion":0,"provider":"flowmusic-web","accessToken":"synthetic"}"#,
    )
    .unwrap();
    std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_DRIVER", "local");
    std::env::set_var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR", objects);
    let document = json!({"providers":[{"id":"flowmusic-web","preset":"producer","base_url":"https://www.flowmusic.app",
        "explicit_credentials_only":true,"credentials":[{"id":"synthetic-account","runtime_state_object_key":key,"supported_models":["producer:standard"]}],
        "supported_models":["producer:standard"],"execution_mode":"direct_http",
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"flowmusic_web"}}],
        "model_routes":[{"pattern":"producer:standard","provider_ids":["flowmusic-web"]}],"aliases":{}});
    let mut fixture =
        support::ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), true);
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root.join("runtime")).await.unwrap());
    let response = Box::pin(
        build_router(fixture.state.clone()).oneshot(
            Request::post("/v1/music/generations")
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"producer:standard","prompt":"Must not generate"}).to_string(),
                ))
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert!(response.status().is_server_error());
    let body = support::parse_json(response).await;
    assert_eq!(body["error"]["code"], "flowmusic_session_invalid");
    assert!(!body.to_string().contains("synthetic"));
    fixture.state.local_runtime.as_ref().unwrap().close().await;
}
