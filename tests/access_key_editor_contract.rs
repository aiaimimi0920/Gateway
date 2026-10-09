//! Management disclosure follows console policy; create/edit keep quota atomic.
#[path = "console_contract_support/mod.rs"]
mod support;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use neuro_gateway::{config::GatewayStorageMode, http::router::build_router, state::AppState};
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use tower::ServiceExt;

const ROOT: &str = "/v1/internal/gateway/access/keys";

async fn api(
    state: &Arc<AppState>,
    path: &str,
    token: &str,
    body: Option<Value>,
    remote: bool,
) -> (StatusCode, HeaderMap, Value) {
    let mut request = if body.is_some() {
        Request::post(path)
    } else {
        Request::get(path)
    }
    .header("content-type", "application/json")
    .header("authorization", format!("Bearer {token}"))
    .header("x-management-token", token)
    .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
    .unwrap();
    let address: SocketAddr = if remote {
        "192.0.2.1:12345"
    } else {
        "127.0.0.1:12345"
    }
    .parse()
    .unwrap();
    request.extensions_mut().insert(ConnectInfo(address));
    let response = build_router(state.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn managed_copy_and_quota_wire_contract() {
    let root = std::env::temp_dir().join(format!("gateway-key-editor-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.console.remote_access_enabled = false;
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
    let management = support::MANAGEMENT_TOKEN;
    let mut input = json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local",
        "resolvedTenantId":"local","keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"limited",
        "quota":{"mode":"message_prepaid","limit":3}});
    let (status, _, created) = api(&state, ROOT, management, Some(input.clone()), false).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().unwrap();
    let token = created["token"].as_str().unwrap();
    let secret_path = format!("{ROOT}/{id}/secret");
    for _ in 0..2 {
        let (status, headers, secret) =
            api(&state, &secret_path, management, Some(json!({})), false).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["cache-control"], "no-store");
        assert_eq!(secret["token"], token);
    }
    for invalid in ["", "wrong", token] {
        let (status, _, body) = api(&state, &secret_path, invalid, Some(json!({})), false).await;
        assert!(!status.is_success());
        assert!(!body.to_string().contains(token));
    }
    assert_eq!(
        api(&state, &secret_path, management, Some(json!({})), true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        api(&state, &secret_path, management, None, false).await.0,
        StatusCode::METHOD_NOT_ALLOWED
    );
    let (_, _, catalog) = api(
        &state,
        "/v1/internal/gateway/access/catalog",
        management,
        None,
        false,
    )
    .await;
    assert!(!catalog.to_string().contains(token));
    assert_eq!(catalog["balances"][0]["remainingMessages"], 3);
    input["displayName"] = json!("must-not-save");
    input["quota"]["limit"] = json!(-1);
    assert_eq!(
        api(
            &state,
            &format!("{ROOT}/{id}"),
            management,
            Some(input.clone()),
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, _, catalog) = api(
        &state,
        "/v1/internal/gateway/access/catalog",
        management,
        None,
        false,
    )
    .await;
    assert_eq!(catalog["accessKeys"][0]["displayName"], "limited");
    input["quota"]["limit"] = json!(10);
    let (status, _, updated) = api(
        &state,
        &format!("{ROOT}/{id}"),
        management,
        Some(input),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(updated["token"].is_null());
    assert!(updated["externalKey"].is_null());
    api(
        &state,
        &format!("{ROOT}/{id}/revoke"),
        management,
        Some(json!({})),
        false,
    )
    .await;
    assert!(
        !api(&state, &secret_path, management, Some(json!({})), false)
            .await
            .0
            .is_success()
    );
    state.local_runtime.as_ref().unwrap().close().await;
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
