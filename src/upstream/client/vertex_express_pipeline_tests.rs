//! Public protocol/auth/routing/finalize with production local bootstrap and a normal prepaid Key.
use super::vertex_express_test_support as wire;
use crate::{access_balance::AccessBalanceStore, db, http::router::build_router, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "vertex_express_pipeline_failures.rs"]
mod failures;
#[path = "vertex_express_pipeline_preview.rs"]
mod preview;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
#[path = "vertex_express_pipeline_tools.rs"]
mod tools;

const MODEL: &str = "public-express";
const KEY: &str = "synthetic-exact-express-key";
struct Fixture {
    state: Arc<AppState>,
    id: String,
    token: String,
}
impl Fixture {
    async fn new(server: &wire::Server) -> Self {
        let root =
            std::env::temp_dir().join(format!("vertex-express-pipeline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let document = json!({"providers":[{"id":"express-pipeline","preset":"vertex-express-api",
            "base_url":crate::protocol::vertex_express::BASE_URL,"explicit_credentials_only":true,
            "request_wait":{"mode":"disabled"},
            "supported_models":[MODEL],"model_map":{MODEL:"gemini-contract"},
            "credentials":[{"id":"unrelated","api_key":"synthetic-unrelated","supported_models":["other-model"]},
                {"id":"exact","api_key":KEY,"supported_models":[MODEL]}]}],"model_routes":[]});
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        eprintln!("Express fixture: production local bootstrap ready");
        Arc::get_mut(&mut state).unwrap().upstream_client.http = server.client();
        let (status, text) = request(&state, "/v1/internal/gateway/access/keys", support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
            "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Express prepaid contract","metadata":{"models":[MODEL]}})).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let key: Value = serde_json::from_str(&text).unwrap();
        let id = key["id"].as_str().unwrap().to_owned();
        let token = key["token"].as_str().unwrap().to_owned();
        let (status, text) = request(
            &state,
            &format!("/v1/internal/gateway/access/keys/{id}/balances/adjust"),
            support::MANAGEMENT_TOKEN,
            json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        Self { state, id, token }
    }
    async fn open(&self, endpoint: &str, body: Value) -> axum::response::Response {
        eprintln!(
            "Express public router: {endpoint} stream={}",
            body["stream"]
        );
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            Box::pin(
                build_router(self.state.clone()).oneshot(
                    Request::post(format!("/v1/{endpoint}"))
                        .header("authorization", format!("Bearer {}", self.token))
                        .header("cookie", "foreign=caller-secret")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                ),
            ),
        )
        .await
        .expect("Express public fixture exceeded its bounded local request deadline")
        .unwrap();
        eprintln!("Express public router returned {}", response.status());
        response
    }
    async fn remaining(&self) -> i64 {
        AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages
            .unwrap()
    }
    async fn audit(&self) -> db::GatewayRequestAuditView {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let rows = self
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == 1 && !matches!(rows[0].status.as_str(), "started" | "running") {
                    // The audit precedes stream quota settlement; wait for the terminal owner, not a desired balance.
                    while Arc::strong_count(&self.state) > 1 {
                        tokio::task::yield_now().await;
                    }
                    return rows.into_iter().next().unwrap();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    async fn finish(self) {
        crate::runtime::mark_runtime_draining(&self.state, "express_pipeline_finish");
        self.state.shutdown.request("express_pipeline_finish");
        tokio::time::timeout(Duration::from_secs(3), async {
            while Arc::strong_count(&self.state) > 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        self.state.local_runtime.as_ref().unwrap().close().await;
        self.state
            .upstream_client
            .freebuff
            .shutdown(Duration::from_secs(1))
            .await
            .unwrap();
    }
}
fn request<'a>(
    state: &'a Arc<AppState>,
    path: &'a str,
    token: &'a str,
    body: Value,
) -> std::pin::Pin<Box<impl std::future::Future<Output = (StatusCode, String)> + 'a>> {
    Box::pin(async move {
        let response = build_router(state.clone())
            .oneshot(
                Request::post(path)
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-management-token", token)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    })
}
fn body(endpoint: &str, stream: bool) -> Value {
    match endpoint {
        "responses" => json!({"model":MODEL,"input":"合成问题","stream":stream}),
        "messages" => {
            json!({"model":MODEL,"max_tokens":32,"messages":[{"role":"user","content":"合成问题"}],"stream":stream})
        }
        _ => {
            json!({"model":MODEL,"messages":[{"role":"user","content":"合成问题"}],"stream":stream})
        }
    }
}
fn event(raw: Value) -> Vec<u8> {
    format!("data: {raw}\n\n").into_bytes()
}
fn success_body() -> Vec<u8> {
    [event(json!({"candidates":[{"index":0,"content":{"parts":[{"text":"合成"}]}}]})),
        event(json!({"candidates":[{"index":0,"content":{"parts":[{"text":"答案"}]},"finishReason":"STOP"}],
            "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}}))].concat()
}
fn assert_wire(server: &wire::Server) {
    assert_wire_attempts(server, 1);
}
fn assert_wire_attempts(server: &wire::Server, max_attempts: usize) {
    let calls = server.calls.lock().unwrap();
    assert!(
        !calls.is_empty() && calls.len() <= max_attempts,
        "{} attempts, maximum {max_attempts}",
        calls.len()
    );
    for (path, headers, raw) in calls.iter() {
        assert_eq!(
            path,
            "/v1/publishers/google/models/gemini-contract:streamGenerateContent?alt=sse"
        );
        assert_eq!(headers["x-goog-api-key"], KEY);
        for name in ["authorization", "cookie", "x-goog-user-project"] {
            assert!(!headers.contains_key(name));
        }
        assert_eq!(raw["contents"][0]["parts"][0]["text"], "合成问题");
    }
}
fn assert_identity(row: &db::GatewayRequestAuditView) {
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some("gemini-contract"));
    assert_eq!(row.provider_account_id.as_deref(), Some("express-pipeline"));
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
}
async fn drain(body: Body) -> String {
    String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap()
}
#[tokio::test]
async fn vertex_express_pipeline_three_public_protocols_debit_and_persist_real_usage() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::start(200, success_body()).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, body(endpoint, stream)).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    assert!(text.contains("合成答案") && !text.contains(KEY), "{text}");
                    if stream {
                        assert!(
                            text.contains(match endpoint {
                                "responses" => "response.completed",
                                "messages" => "message_stop",
                                _ => "[DONE]",
                            }),
                            "{text}"
                        );
                    }
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "completed");
                    assert_eq!(
                        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                        (Some(2), Some(3), Some(5))
                    );
                    assert_identity(&row);
                    assert_wire(&server);
                    assert_eq!(fixture.remaining().await, 1);
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}
#[path = "vertex_express_pipeline_signed.rs"]
mod signed;
