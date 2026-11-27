//! Real Gateway router, SQLite and in-process pure-Rust Cursor runtime; synthetic upstream only.
use super::runtime::{
    worker::{WorkerOutput, WorkerToolCall},
    Runtime,
};
use crate::{db, state::AppState};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

async fn app(enabled: bool) -> Arc<AppState> {
    app_worker(
        enabled,
        Runtime::fixture(|input, hooks| {
            Box::pin(async move {
                if !input["prompt"]
                    .as_str()
                    .is_some_and(|p| p.contains("hello"))
                {
                    return Err(super::error("worker_failed"));
                }
                hooks.authorize().await?;
                hooks.sent()?;
                Ok(WorkerOutput {
                    usage: None,
                    text: "remote fixture".into(),
                    thinking: None,
                    requested_model: "default".into(),
                })
            })
        }),
    )
    .await
}

async fn app_worker(enabled: bool, runtime: Runtime) -> Arc<AppState> {
    let root = std::path::Path::new("C:/Users/Public/nas_home/AI/GameEditor/linshi")
        .join(format!("cursor-remote-http-{}", uuid::Uuid::new_v4()));
    #[cfg(not(windows))]
    let root = std::env::temp_dir().join(root.file_name().unwrap());
    std::fs::create_dir_all(&root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = crate::config::GatewayStorageMode::Local;
    config.gateway_api_key = None;
    config.gateway_api_key_secret = None;
    config.console.management_ui_enabled = false;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    let expiry = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    let token =
        crate::protocol::cursor_auth::tests::token("github|fixture", expiry.unix_timestamp());
    let document = json!({"providers":[{"id":"cursor-remote","auth_mode":"cursor_remote","base_url":"",
        "explicit_credentials_only":true,"credentials":[{"id":"cursor-account","enabled":enabled,
            "account_name":"owner@example.test","auth_token":token,
            "expires_at":expiry.format(&time::format_description::well_known::Rfc3339).unwrap()}]}]});
    std::fs::write(
        &config.console.routes_file,
        serde_yaml::to_string(&document).unwrap(),
    )
    .unwrap();
    let mut state = Box::pin(crate::runtime::build_app_state(config))
        .await
        .unwrap();
    Arc::get_mut(&mut state).unwrap().lifecycle.cursor_remote = runtime;
    state
}

async fn key(state: &AppState, scope: &str, provider: &str) -> String {
    state
        .local_runtime
        .as_ref()
        .unwrap()
        .save_access_key(
            None,
            db::UpsertAccessKeyInput {
                owner_type: "user".into(),
                owner_id: "fixture".into(),
                resolved_project_id: "local".into(),
                resolved_tenant_id: "local".into(),
                key_kind: "normal".into(),
                public_key_prefix: "sk-gw".into(),
                display_name: "Cursor remote test".into(),
                expires_at: None,
                bundle_ids: vec![],
                metadata: Some(
                    json!({"scopes":[scope],"models":["cursor-personal"],"providerIds":[provider]}),
                ),
            },
        )
        .await
        .unwrap()
        .token
        .unwrap()
}

async fn post(state: &Arc<AppState>, token: &str, body: Value) -> (StatusCode, String) {
    post_to(state, token, "/v1/responses", body).await
}

async fn post_to(
    state: &Arc<AppState>,
    token: &str,
    endpoint: &str,
    body: Value,
) -> (StatusCode, String) {
    let request = Request::post(endpoint)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = Box::pin(crate::http::router::build_router(state.clone()).oneshot(request))
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[path = "http_chat_tests.rs"]
mod chat;
#[path = "http_custom_tool_tests.rs"]
mod custom_tools;
#[path = "http_image_tests.rs"]
mod images;
#[path = "http_metered_tests.rs"]
mod metered;
#[path = "http_native_tests.rs"]
mod native;
#[path = "http_startup_tests.rs"]
mod startup;
#[path = "http_thinking_tests.rs"]
mod thinking;
#[path = "http_tool_tests.rs"]
mod tools;
#[path = "http_usage_tests.rs"]
mod usage;
#[path = "websocket_tests.rs"]
mod websocket;

fn run<F: std::future::Future<Output = ()> + 'static>(test: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(test());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cursor_remote_http_responses_json_sse_exact_account_and_no_fallback() {
    run(|| async {
        let state = app(true).await;
        let token = key(&state, "responses", "cursor-remote").await;
        for stream in [false, true] {
            let (status, body) = post(
                &state,
                &token,
                json!({"model":"cursor-personal","input":"hello","store":false,"stream":stream}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert!(body.contains("remote fixture"), "{body}");
            if stream {
                for event in [
                    "response.created",
                    "response.output_text.delta",
                    "response.completed",
                ] {
                    assert!(body.contains(event), "{body}");
                }
            } else {
                assert_eq!(
                    serde_json::from_str::<Value>(&body).unwrap()["status"],
                    "completed"
                );
            }
        }
        let wrong = key(&state, "responses", "not-cursor").await;
        let (status, _) = post(
            &state,
            &wrong,
            json!({"model":"cursor-personal","input":"hello","store":false}),
        )
        .await;
        assert_ne!(status, StatusCode::OK);
        let (status, _) = post(
            &state,
            &token,
            json!({"model":"cursor-personal","input":"hello","store":true}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let rows = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let rows = state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == 4 && rows.iter().all(|r| r.completed_at.is_some()) {
                    break rows;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let mut sent = 0;
        for row in rows {
            let trace = row.route_trace.unwrap();
            if trace["routeAttemptCount"] == 1 {
                sent += 1;
                assert_eq!(row.provider_account_id.as_deref(), Some("cursor-remote"));
                assert_eq!(trace["realCredentialRef"], "cursor-account");
                assert_eq!(
                    trace["routeRevision"],
                    state.route_config.snapshot().revision().id()
                );
                assert_eq!(trace["cursorRemoteDeliveryUnknown"], false);
                assert_eq!(row.upstream_status, Some(200));
                assert!(row.total_tokens.is_none());
            } else {
                assert_eq!(trace["routeAttemptCount"], 0);
                assert!(row.provider_account_id.is_none());
            }
        }
        assert_eq!(sent, 2);
        let compiled = state.route_config.snapshot().get_providers();
        assert!(!compiled[0].credential_pool[0].enabled);
        assert!(compiled[0].credential_pool[0].payload.auth_token.is_none());
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_http_imported_disabled_account_never_dispatches() {
    run(|| async {
        let state = app(false).await;
        let token = key(&state, "responses", "cursor-remote").await;
        let (status, body) = post(
            &state,
            &token,
            json!({"model":"cursor-personal","input":"hello","store":false}),
        )
        .await;
        assert_ne!(status, StatusCode::OK);
        assert!(body.contains("cursor_remote_account_not_authorized"));
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
