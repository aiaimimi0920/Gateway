//! Public auth -> preset/route/exact Key -> actual sender -> caller protocol -> durable finalize.
use super::kiro_api_key_test_support as wire;
use crate::{db, http::router::build_router, local_runtime::LocalRuntime};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "kiro_api_key_pipeline_failures.rs"]
mod failures;
#[path = "kiro_api_key_pipeline_integrity.rs"]
mod integrity;
#[path = "kiro_api_key_pipeline_streams.rs"]
mod streams;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
#[path = "kiro_api_key_pipeline_tools.rs"]
mod tools;

fn document() -> Value {
    let provider = json!({"id":"key-pipeline","preset":"kiro-api-key","base_url":crate::protocol::kiro_api_key::BASE_URL,
        "explicit_credentials_only":true,"supported_models":["public-alias"],"model_map":{"public-alias":"claude-sonnet-contract"},
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"kiro"},
        "credentials":[{"id":"unrelated","api_key":"synthetic-unrelated-key","supported_models":["another-model"]},
            {"id":"exact","api_key":"synthetic-exact-key","supported_models":["public-alias"]}]});
    let mut backup = provider.clone();
    backup["id"] = json!("key-backup");
    backup["credentials"][0]["id"] = json!("backup-unrelated");
    backup["credentials"][1]["id"] = json!("backup-exact");
    backup["credentials"][1]["api_key"] = json!("synthetic-backup-key");
    json!({"providers":[provider,backup],"model_routes":[{"pattern":"public-alias","provider_ids":["key-pipeline","key-backup"]}]})
}
fn run<F: Future<Output = ()> + 'static>(test: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(60), test())
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
struct Fixture {
    console: support::ConsoleStateFixture,
    local: LocalRuntime,
}
impl Fixture {
    async fn new(server: &wire::Server) -> Self {
        let mut console =
            support::ConsoleStateFixture::new(serde_json::from_value(document()).unwrap(), false);
        let local = LocalRuntime::open(
            &std::env::temp_dir().join(format!("kiro-key-pipeline-{}", uuid::Uuid::new_v4())),
        )
        .await
        .unwrap();
        let state = Arc::get_mut(&mut console.state).unwrap();
        state.local_runtime = Some(local.clone());
        state.upstream_client = super::UpstreamClient::new(5);
        state.upstream_client.http = server.client();
        Self { console, local }
    }
    async fn open(
        &self,
        endpoint: &str,
        body: Value,
        authorized: bool,
    ) -> axum::response::Response {
        build_router(self.console.state.clone())
            .oneshot(
                Request::post(format!("/v1/{endpoint}"))
                    .header(
                        "authorization",
                        if authorized {
                            "Bearer public-api-contract-secret"
                        } else {
                            "Bearer invalid-key"
                        },
                    )
                    .header("cookie", "foreign=caller-secret")
                    .header("user-agent", "caller-UA")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    async fn audit(&self) -> db::GatewayRequestAuditView {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let rows = self
                    .local
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if !rows.is_empty()
                    && rows
                        .iter()
                        .all(|a| !matches!(a.status.as_str(), "started" | "running"))
                {
                    assert_eq!(rows.len(), 1);
                    return rows.into_iter().next().unwrap();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    async fn finish(self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while Arc::strong_count(&self.console.state) > 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(self.console);
        self.local.close().await;
    }
}
fn body(endpoint: &str, stream: bool) -> Value {
    if endpoint == "responses" {
        json!({"model":"public-alias","input":"合成问题","instructions":"rules","stream":stream})
    } else if endpoint == "messages" {
        json!({"model":"public-alias","system":"rules","max_tokens":32,"messages":[{"role":"user","content":"合成问题"}],"stream":stream})
    } else {
        json!({"model":"public-alias","messages":[{"role":"system","content":"rules"},{"role":"user","content":"合成问题"}],"stream":stream})
    }
}
fn assert_wire(server: &wire::Server) {
    let calls = server.calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "No account retry or channel fallback");
    assert_eq!(server.attempts(), 1);
    let (headers, body) = &calls[0];
    assert_eq!(headers["authorization"], "Bearer synthetic-exact-key");
    assert_eq!(headers["tokentype"], "API_KEY");
    assert_eq!(
        headers["x-amz-target"],
        crate::protocol::kiro_api_key::TARGET
    );
    assert_eq!(headers["amz-sdk-request"], "attempt=1; max=1");
    assert!(!headers.contains_key("cookie"));
    assert!(headers["user-agent"]
        .to_str()
        .unwrap()
        .contains("AmazonQ-For-CLI"));
    let state = &body["conversationState"];
    let current = &state["currentMessage"]["userInputMessage"];
    assert_eq!(current["modelId"], "claude-sonnet-contract");
    assert_eq!(current["origin"], "KIRO_CLI");
    assert_eq!(current["content"], "合成问题");
    assert_eq!(state["history"][0]["userInputMessage"]["content"], "rules");
    assert!(state.get("agentContinuationId").is_none());
    assert!(body.get("profileArn").is_none());
}
fn assert_identity(row: &db::GatewayRequestAuditView) {
    assert_eq!(row.requested_model.as_deref(), Some("public-alias"));
    assert_eq!(
        row.resolved_model.as_deref(),
        Some("claude-sonnet-contract")
    );
    assert_eq!(row.provider_account_id.as_deref(), Some("key-pipeline"));
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
}
async fn drain(mut body: Body) -> (String, bool) {
    let mut bytes = Vec::new();
    let mut failed = false;
    while let Some(frame) = body.frame().await {
        match frame {
            Ok(frame) => {
                if let Ok(data) = frame.into_data() {
                    bytes.extend_from_slice(&data);
                }
            }
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    (String::from_utf8(bytes).unwrap(), failed)
}
#[test]
fn kiro_api_key_pipeline_public_three_endpoints_json_and_sse() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            let server = wire::Server::stream(wire::event_frame("合成答案"), false).await;
            let fixture = Fixture::new(&server).await;
            wire::within(&server.origin, async {
                let response = fixture.open(endpoint, body(endpoint, stream), true).await;
                assert_eq!(response.status(), StatusCode::OK);
                let (text, failed) = drain(response.into_body()).await;
                assert!(!failed, "{text}");
                assert!(text.contains("合成答案"), "{text}");
                assert!(!text.contains("synthetic-exact-key"));
                assert_wire(&server);
                if stream {
                    assert!(
                        text.contains(match endpoint {
                            "responses" => "response.completed",
                            "messages" => "message_stop",
                            _ => "[DONE]",
                        }),
                        "{text}"
                    );
                } else {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    assert_eq!(value["model"], "public-alias");
                    match endpoint {
                        "responses" => {
                            assert_eq!(value["status"], "completed");
                            assert_eq!(value["output"][0]["content"][0]["text"], "合成答案");
                        }
                        "messages" => {
                            assert_eq!(value["stop_reason"], "end_turn");
                            assert_eq!(value["content"][0]["text"], "合成答案");
                        }
                        _ => assert_eq!(value["choices"][0]["message"]["content"], "合成答案"),
                    }
                }
                let audit = fixture.audit().await;
                assert_eq!(audit.status, "completed");
                assert_identity(&audit);
                // Existing Kiro decoder estimates tokens, not upstream metering/subscription credits.
                assert!(audit.prompt_tokens.is_some_and(|v| v > 0));
                assert!(audit.completion_tokens.is_some_and(|v| v > 0));
                // Stream finalization commits audit before model health in its owned task.
                let states = tokio::time::timeout(Duration::from_secs(3), async {
                    loop {
                        let states = fixture
                            .local
                            .list_model_states(db::CredentialModelStateFilters::default())
                            .await
                            .unwrap();
                        if !states.is_empty() {
                            break states;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                assert!(
                    states
                        .iter()
                        .any(|s| s.provider_credential_ref.as_deref() == Some("exact")
                            && s.model == "claude-sonnet-contract"
                            && s.status == "active"),
                    "{states:?}"
                );
                assert!(!states.iter().any(|s| s
                    .provider_credential_ref
                    .as_deref()
                    .is_some_and(|id| id != "exact")));
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    }
}
