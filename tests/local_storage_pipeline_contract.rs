//! Loopback-only product flow: management, authorization, billing, stream completion and restart.
#[path = "console_contract_support/mod.rs"]
mod support;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use http_body_util::BodyExt;
use neuro_gateway::{
    access_balance::AccessBalanceStore, config::GatewayStorageMode, http::router::build_router,
    state::AppState,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower::ServiceExt;

async fn upstream(
    State(calls): State<Arc<AtomicUsize>>,
    Json(request): Json<Value>,
) -> axum::response::Response {
    calls.fetch_add(1, Ordering::SeqCst);
    if request.to_string().contains("fail-upstream") {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":{"message":"fixture unavailable"}})),
        )
            .into_response();
    }
    if request["stream"] == true {
        return ([("content-type", "text/event-stream")], concat!(
            "data: {\"id\":\"fixture\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"fixture\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n",
            "data: [DONE]\n\n")).into_response();
    }
    Json(json!({"id":"fixture","object":"chat.completion","model":"nvidia-test",
        "choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})).into_response()
}

async fn request(
    state: &Arc<AppState>,
    path: &str,
    key: &str,
    body: Value,
) -> (StatusCode, String) {
    let request = Request::post(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {key}"))
        .header("x-management-token", key)
        .body(Body::from(body.to_string()))
        .unwrap();
    eprintln!("[local-storage-stack] request: dispatch");
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        build_router(state.clone()).oneshot(request),
    )
    .await
    .unwrap()
    .unwrap();
    eprintln!("[local-storage-stack] request: response received");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    eprintln!("[local-storage-stack] request: body collected");
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn remaining(state: &Arc<AppState>, id: &str) -> i64 {
    eprintln!("[local-storage-stack] balance: read");
    AccessBalanceStore::from_state(state)
        .unwrap()
        .get(id)
        .await
        .unwrap()
        .unwrap()
        .remaining_messages
        .unwrap()
}

#[tokio::test]
async fn local_mode_ignores_external_databases_and_enforces_http_balances() {
    eprintln!("[local-storage-stack] flow: entered");
    let root = std::env::temp_dir().join(format!("gateway-local-flow-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(upstream))
        .with_state(calls.clone());
    let upstream_task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.redis_url = "deliberately invalid Redis URL".into();
    config.database_url = Some("deliberately invalid PostgreSQL URL".into());
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    std::fs::write(&config.console.routes_file, format!("providers:\n  - id: nvidia-fixture\n    base_url: http://{address}\n    api_key: fixture-only\n    supported_models: [nvidia-test]\nmodel_routes: []\n")).unwrap();
    eprintln!("[local-storage-stack] initial state: build");
    let state = neuro_gateway::runtime::build_app_state(config.clone())
        .await
        .unwrap();
    eprintln!("[local-storage-stack] initial state: ready");
    assert!(state.pg_pool.is_none());
    assert!(state.redis_pool.is_closed());
    assert!(state.auth_adapters.is_empty());
    eprintln!("[local-storage-stack] create key: begin");
    let (status, created) = request(&state, "/v1/internal/gateway/access/keys", support::MANAGEMENT_TOKEN,
        json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
            "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"flow","metadata":{"models":["nvidia-test"]}})).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let created: Value = serde_json::from_str(&created).unwrap();
    let id = created["id"].as_str().unwrap();
    let token = created["token"].as_str().unwrap();
    let adjust_path = format!("/v1/internal/gateway/access/keys/{id}/balances/adjust");
    eprintln!("[local-storage-stack] initialize balance: begin");
    let (status, adjusted) = request(
        &state,
        &adjust_path,
        support::MANAGEMENT_TOKEN,
        json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{adjusted}");
    let payload = |model: &str, stream: bool, content: &str| json!({"model":model,"stream":stream,"messages":[{"role":"user","content":content}]});
    eprintln!("[local-storage-stack] reject forbidden model: begin");
    let (status, _) = request(
        &state,
        "/v1/chat/completions",
        token,
        payload("forbidden", false, "hello"),
    )
    .await;
    assert!(!status.is_success());
    assert_eq!(remaining(&state, id).await, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    eprintln!("[local-storage-stack] nonstream request: begin");
    let (status, body) = request(
        &state,
        "/v1/chat/completions",
        token,
        payload("nvidia-test", false, "hello"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(remaining(&state, id).await, 1);
    eprintln!("[local-storage-stack] stream request: begin");
    let (status, body) = request(
        &state,
        "/v1/chat/completions",
        token,
        payload("nvidia-test", true, "hello"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("[DONE]"), "{body}");
    for _ in 0..100 {
        if remaining(&state, id).await == 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(remaining(&state, id).await, 0);
    eprintln!("[local-storage-stack] exhausted balance request: begin");
    let (status, body) = request(
        &state,
        "/v1/chat/completions",
        token,
        payload("nvidia-test", false, "hello"),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body.contains("message_balance_exhausted"), "{body}");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    eprintln!("[local-storage-stack] restart: close initial runtime");
    let local = state.local_runtime.as_ref().unwrap();
    local.close().await;
    drop(state);
    eprintln!("[local-storage-stack] restarted state: build");
    let state = neuro_gateway::runtime::build_app_state(config)
        .await
        .unwrap();
    eprintln!("[local-storage-stack] restarted state: ready");
    assert_eq!(remaining(&state, id).await, 0);
    eprintln!("[local-storage-stack] replenish balance: begin");
    let (status, body) = request(
        &state,
        &adjust_path,
        support::MANAGEMENT_TOKEN,
        json!({"messageDelta":1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    eprintln!("[local-storage-stack] failed upstream request: begin");
    let (status, _) = request(
        &state,
        "/v1/chat/completions",
        token,
        payload("nvidia-test", false, "fail-upstream"),
    )
    .await;
    assert!(!status.is_success());
    assert_eq!(remaining(&state, id).await, 1);
    let balance = AccessBalanceStore::from_state(&state)
        .unwrap()
        .get(id)
        .await
        .unwrap()
        .unwrap();
    eprintln!("[local-storage-stack] final balance: read complete");
    assert_eq!(balance.total_messages, Some(3));
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    eprintln!("[local-storage-stack] flow: cleanup");
    upstream_task.abort();
    let _ = upstream_task.await;
    for attempt in 0..40 {
        match std::fs::remove_dir_all(&root) {
            Ok(()) => break,
            Err(_) if attempt < 39 => {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await
            }
            Err(error) => panic!("fixture cleanup failed: {error}"),
        }
    }
}

#[test]
fn config_local_mode_needs_no_redis_and_server_mode_still_requires_it() {
    const KEYS: [&str; 5] = [
        "GATEWAY_STORAGE_MODE",
        "GATEWAY_REDIS_URL",
        "GATEWAY_RUNTIME_ROLE",
        "GATEWAY_DESKTOP_MANAGED",
        "GATEWAY_CONSOLE_STORAGE",
    ];
    struct Restore(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
    let _restore = Restore(
        KEYS.iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect(),
    );
    for key in KEYS {
        std::env::remove_var(key);
    }
    std::env::set_var("GATEWAY_STORAGE_MODE", "local");
    let config = neuro_gateway::config::Config::from_env().unwrap();
    assert!(config.redis_url.is_empty());
    assert_eq!(config.storage_mode, GatewayStorageMode::Local);
    assert_eq!(
        config.runtime_role,
        neuro_gateway::config::GatewayRuntimeRole::Standalone
    );
    std::env::set_var("GATEWAY_STORAGE_MODE", "server");
    assert!(neuro_gateway::config::Config::from_env()
        .unwrap_err()
        .contains("GATEWAY_REDIS_URL"));
}

#[tokio::test]
async fn local_readiness_checks_sqlite_before_any_provider_is_configured() {
    let root = std::env::temp_dir().join(format!("gateway-local-flow-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.gateway_api_key = None;
    config.gateway_api_key_secret = None;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    std::fs::write(
        &config.console.routes_file,
        "providers: []\nmodel_routes: []\n",
    )
    .unwrap();
    let state = neuro_gateway::runtime::build_app_state(config)
        .await
        .unwrap();
    let response = build_router(state.clone())
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = support::parse_json(response).await;
    assert_eq!(json["storage_backend"], "sqlite");
    assert_eq!(json["degraded"], false);
    assert_eq!(json["redis_required"], false);
    assert_eq!(json["routing_configured"], false);
    state.local_runtime.as_ref().unwrap().close().await;
    let response = build_router(state.clone())
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    drop(state);
    for attempt in 0..40 {
        match std::fs::remove_dir_all(&root) {
            Ok(()) => break,
            Err(_) if attempt < 39 => {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await
            }
            Err(error) => panic!("fixture cleanup failed: {error}"),
        }
    }
}
