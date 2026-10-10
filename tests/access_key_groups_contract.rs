//! Loopback contract: multi-group keys never select accounts outside the live group union.
#[path = "console_contract_support/mod.rs"]
mod support;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use http_body_util::BodyExt;
use neuro_gateway::{
    config::GatewayStorageMode, http::router::build_router, routing::config::RouteConfigYaml,
    state::AppState,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const ROOT: &str = "/v1/internal/gateway/access/keys";

fn input(groups: Value) -> Value {
    json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
        "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"group-key","metadata":{"accountGroupIds":groups}})
}

fn api<'a>(
    state: &'a Arc<AppState>,
    path: &'a str,
    token: &'a str,
    body: Option<Value>,
) -> std::pin::Pin<Box<impl std::future::Future<Output = (StatusCode, Value)> + 'a>> {
    // Router service futures are large in debug builds; callers retain only this pointer.
    Box::pin(async move {
        let builder = if body.is_some() {
            Request::post(path)
        } else {
            Request::get(path)
        };
        let request = builder
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-management-token", token)
            .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
            .unwrap();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            build_router(state.clone()).oneshot(request),
        )
        .await
        .unwrap()
        .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes))),
        )
    })
}

async fn models(state: &Arc<AppState>, token: &str) -> Vec<String> {
    let (status, result) = api(state, "/v1/models", token, None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    result["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| model["id"].as_str().unwrap().to_owned())
        .collect()
}

async fn relay(state: &Arc<AppState>, token: &str, model: &str) -> StatusCode {
    api(
        state,
        "/v1/chat/completions",
        token,
        Some(json!({"model":model,"messages":[{"role":"user","content":"fixture"}]})),
    )
    .await
    .0
}

async fn upstream(
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    calls.lock().unwrap().push(
        headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned(),
    );
    Json(
        json!({"id":"fixture","object":"chat.completion","model":body["model"],
        "choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
    )
}

#[test]
fn group_metadata_is_bounded_and_distinguishes_legacy_from_deny_all() {
    use neuro_gateway::access_key_groups::group_ids;
    assert_eq!(group_ids(None).unwrap(), None);
    assert_eq!(
        group_ids(Some(&json!({"accountGroupIds":[]}))).unwrap(),
        Some(vec![])
    );
    assert_eq!(
        group_ids(Some(&json!({"accountGroupIds":["a","a","b"]}))).unwrap(),
        Some(vec!["a".into(), "b".into()])
    );
    for value in [
        Value::Null,
        json!("a"),
        json!([1]),
        json!([""]),
        json!([" a"]),
        json!(["a\n"]),
        json!(vec!["a"; 129]),
    ] {
        assert!(group_ids(Some(&json!({"accountGroupIds":value}))).is_err());
    }
}

#[tokio::test]
async fn multi_group_keys_filter_models_accounts_edits_rotation_and_live_membership() {
    let root = std::env::temp_dir().join(format!("gateway-key-groups-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(upstream))
        .with_state(calls.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    let yaml = format!(
        r#"
providers:
  - id: nvidia-fixture
    base_url: http://{address}
    supported_models: [model-a, model-b, model-c, shared]
    credentials:
      - id: account-a
        api_key: fixture-a
        supported_models: [model-a, shared]
      - id: account-b
        api_key: fixture-b
        supported_models: [model-b, shared]
      - id: account-c
        api_key: fixture-c
        supported_models: [model-c, shared]
account_groups:
  - id: group-a
    name: Group A
    provider_credential_ids: [account-a]
  - id: group-b
    name: Group B
    provider_credential_ids: [account-b]
  - id: group-disabled
    name: Disabled
    enabled: false
    provider_credential_ids: [account-c]
aliases:
  alias-a: model-a
model_routes: []
"#
    );
    std::fs::write(&config.console.routes_file, &yaml).unwrap();
    // Keep the large setup future off the default Windows test thread stack.
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config.clone()))
        .await
        .unwrap();
    let (_, catalog) = api(
        &state,
        "/v1/internal/gateway/access/catalog",
        support::MANAGEMENT_TOKEN,
        None,
    )
    .await;
    let groups = catalog["accountGroups"].as_array().unwrap();
    assert_eq!(groups.len(), 4);
    assert!(groups.iter().any(|group| group["id"] == "default"));
    assert!(!catalog["accountGroups"].to_string().contains("fixture-a"));
    for groups in [json!(["missing"]), json!(["group-disabled"]), json!([1])] {
        assert_eq!(
            api(&state, ROOT, support::MANAGEMENT_TOKEN, Some(input(groups)))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, key) = api(
        &state,
        ROOT,
        support::MANAGEMENT_TOKEN,
        Some(input(json!(["group-a", "group-b"]))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{key}");
    let token = key["token"].as_str().unwrap();
    let id = key["id"].as_str().unwrap();
    let visible = models(&state, token).await;
    for model in ["model-a", "model-b", "shared", "alias-a"] {
        assert!(visible.contains(&model.to_owned()), "{visible:?}");
    }
    assert!(!visible.contains(&"model-c".to_owned()));
    let response = build_router(state.clone())
        .oneshot(
            Request::get("/v1/models")
                .header("x-api-key", token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for model in [
        "model-a", "model-b", "alias-a", "shared", "shared", "shared", "shared",
    ] {
        assert_eq!(relay(&state, token, model).await, StatusCode::OK, "{model}");
    }
    assert_eq!(relay(&state, token, "model-c").await, StatusCode::FORBIDDEN);
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .all(|key| key == "Bearer fixture-a" || key == "Bearer fixture-b"));
    let before = calls.lock().unwrap().len();
    let response = build_router(state.clone())
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", format!("Bearer {token}"))
                .header("x-neuro-account-group", "group-disabled")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"model-c","messages":[]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!response.status().is_success());
    assert_eq!(calls.lock().unwrap().len(), before);

    let (status, edited) = api(
        &state,
        &format!("{ROOT}/{id}"),
        support::MANAGEMENT_TOKEN,
        Some(input(json!(["group-b"]))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert!(edited["token"].is_null());
    assert_eq!(relay(&state, token, "model-a").await, StatusCode::FORBIDDEN);
    assert_eq!(relay(&state, token, "model-b").await, StatusCode::OK);
    let (_, rotated) = api(
        &state,
        &format!("{ROOT}/{id}/rotate"),
        support::MANAGEMENT_TOKEN,
        Some(json!({})),
    )
    .await;
    let replacement = rotated["token"].as_str().unwrap();
    assert_eq!(rotated["metadata"]["accountGroupIds"], json!(["group-b"]));
    assert_eq!(
        api(&state, "/v1/models", token, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(relay(&state, replacement, "model-b").await, StatusCode::OK);

    let mut document: RouteConfigYaml = serde_yaml::from_str(&yaml).unwrap();
    document.account_groups[1].enabled = Some(false);
    state
        .route_config
        .replace_document(document.clone())
        .unwrap();
    assert!(models(&state, replacement).await.is_empty());
    assert_eq!(
        relay(&state, replacement, "model-b").await,
        StatusCode::FORBIDDEN
    );
    document.account_groups[1].enabled = Some(true);
    document.account_groups[1].provider_credential_ids = vec!["account-c".into()];
    state
        .route_config
        .replace_document(document.clone())
        .unwrap();
    assert_eq!(
        relay(&state, replacement, "model-b").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(relay(&state, replacement, "model-c").await, StatusCode::OK);
    document
        .account_groups
        .retain(|group| group.id != "group-b");
    state
        .route_config
        .replace_document(document.clone())
        .unwrap();
    assert!(models(&state, replacement).await.is_empty());
    assert_eq!(
        relay(&state, replacement, "model-c").await,
        StatusCode::FORBIDDEN
    );

    // replace_document changes only the runtime snapshot. Restart restores the
    // persisted original routes; the edited and rotated key must still bind B.
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
        .await
        .unwrap();
    let after_restart = models(&state, replacement).await;
    assert_eq!(after_restart, vec!["model-b", "shared"]);
    assert_eq!(relay(&state, replacement, "model-b").await, StatusCode::OK);
    for model in ["model-a", "model-c"] {
        assert_eq!(
            relay(&state, replacement, model).await,
            StatusCode::FORBIDDEN
        );
    }
    let (_, empty) = api(
        &state,
        ROOT,
        support::MANAGEMENT_TOKEN,
        Some(input(json!([]))),
    )
    .await;
    assert!(models(&state, empty["token"].as_str().unwrap())
        .await
        .is_empty());
    let mut legacy_input = input(json!([]));
    legacy_input.as_object_mut().unwrap().remove("metadata");
    let (_, legacy) = api(&state, ROOT, support::MANAGEMENT_TOKEN, Some(legacy_input)).await;
    assert!(models(&state, legacy["token"].as_str().unwrap())
        .await
        .contains(&"model-c".to_owned()));
    state.local_runtime.as_ref().unwrap().close().await;
    drop(state);
    task.abort();
    let _ = task.await;
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
fn default_accounts_restrict_declared_models_without_breaking_aliases_or_mappings() {
    use neuro_gateway::routing::config::RouteConfigStore;
    let document = serde_json::from_value(json!({
        "providers":[{"id":"nvidia-fixture","base_url":"http://127.0.0.1:1",
            "api_key":"fixture","supported_models":["model-a"],
            "model_map":{"mapped-a":"model-a"},
            "model_map_targets":{"multi-a":["model-a"]}}],
        "account_groups":[{"id":"group-a","name":"A",
            "provider_credential_ids":["nvidia-fixture::default"]}],
        "aliases":{"alias-a":"model-a"}
    }))
    .unwrap();
    let store = RouteConfigStore::from_document(document).unwrap();
    let snapshot = store.snapshot();
    let constraint = snapshot.access_key_group_constraint(&["group-a".into()]);
    for model in ["model-a", "alias-a", "mapped-a", "multi-a"] {
        assert_eq!(
            snapshot
                .resolve_candidates_with_constraint(Some(model), Some(&constraint))
                .candidates
                .len(),
            1,
            "{model}"
        );
    }
    assert!(snapshot
        .resolve_candidates_with_constraint(Some("model-b"), Some(&constraint))
        .candidates
        .is_empty());
    assert_eq!(
        snapshot
            .resolve_candidates_with_constraint(Some("model-b"), None)
            .candidates
            .len(),
        1
    );
}
