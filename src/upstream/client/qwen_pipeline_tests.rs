//! Public auth -> route -> ordinary workspace Qwen -> client protocol -> durable finalization.
use super::qwen_pipeline_test_support as wire;
use crate::{db, http::router::build_router, local_runtime::LocalRuntime};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

const BASE: &str = "https://workspace-a.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1";
fn document() -> Value {
    json!({"providers":[{"id":"qwen-pipeline","preset":"qwen-official-api","base_url":BASE,
        "explicit_credentials_only":true,"supported_models":["public-alias"],
        "model_map":{"public-alias":"qwen-synthetic"},
        "call_flow":{"call_adapter":"openai","fingerprint_adapter":"codex_cli","special_adapter":"none"},
        "credentials":[{"id":"unrelated","api_key":"synthetic-unrelated-key","supported_models":["another-model"]},
            {"id":"exact","api_key":"synthetic-exact-key","base_url":BASE.replace("workspace-a","workspace-b"),"supported_models":["public-alias"]}]}],
        "model_routes":[{"pattern":"public-alias","provider_ids":["qwen-pipeline"]}]})
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
        let directory =
            std::env::temp_dir().join(format!("qwen-pipeline-{}", uuid::Uuid::new_v4()));
        let local = LocalRuntime::open(&directory).await.unwrap();
        let state = Arc::get_mut(&mut console.state).unwrap();
        state.local_runtime = Some(local.clone());
        state.upstream_client = super::UpstreamClient::new(5);
        state.upstream_client.http = server.client();
        Self { console, local }
    }
    async fn request(
        &self,
        server: &wire::Server,
        endpoint: &str,
        authorized: bool,
    ) -> (StatusCode, Value) {
        let body = if endpoint == "responses" {
            json!({"model":"public-alias","input":"Reply OK","stream":false,"max_output_tokens":32})
        } else {
            json!({"model":"public-alias","messages":[{"role":"user","content":"Reply OK"}],"stream":false,"max_tokens":32})
        };
        let request = Request::post(format!("/v1/{endpoint}"))
            .header(
                "authorization",
                if authorized {
                    "Bearer public-api-contract-secret"
                } else {
                    "Bearer invalid-key"
                },
            )
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = wire::within(
            &server.origin,
            build_router(self.console.state.clone()).oneshot(request),
        )
        .await
        .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    async fn audits(&self) -> Vec<db::GatewayRequestAuditView> {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let audits = self
                    .local
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if !audits.is_empty() && audits.iter().all(|a| a.status != "started") {
                    return audits;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    async fn finish(self) {
        let local = self.local;
        tokio::time::timeout(Duration::from_secs(3), async {
            while Arc::strong_count(&self.console.state) > 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(self.console);
        local.close().await;
    }
}
fn assert_wire(server: &wire::Server) {
    let calls = server.calls();
    assert_eq!(calls.len(), 1, "No hidden account or channel retry");
    let call = &calls[0];
    assert_eq!(
        call.path,
        "/workspace-b.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1/chat/completions"
    );
    assert_eq!(call.authorization, "Bearer synthetic-exact-key");
    assert!(call.user_agent.contains("codex"));
    assert_eq!(call.body["model"], "qwen-synthetic");
    assert_eq!(call.body["stream"], false);
    assert_eq!(call.body["max_tokens"], 32);
    assert!(call.body["messages"].is_array());
    assert!(call.body.get("input").is_none());
}
#[test]
fn qwen_pipeline_responses_buffered_keeps_chat_wire_output_and_usage() {
    run(responses);
}
async fn responses() {
    success("responses").await;
}
#[test]
fn qwen_pipeline_chat_buffered_keeps_exact_account_wire_and_usage() {
    run(chat);
}
async fn chat() {
    success("chat/completions").await;
}
async fn success(endpoint: &str) {
    let server = wire::Server::start(wire::completed().to_string(), 200, false).await;
    let fixture = Fixture::new(&server).await;
    let (status, body) = fixture.request(&server, endpoint, true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_wire(&server);
    if endpoint == "responses" {
        assert_eq!(body["object"], "response");
        assert_eq!(body["status"], "completed");
        assert_eq!(body["output"][0]["content"][0]["text"], "OK", "{body}");
        assert_eq!(body["usage"]["input_tokens"], 8);
        assert_eq!(body["usage"]["input_tokens_details"]["cached_tokens"], 3);
    } else {
        assert_eq!(body["choices"][0]["message"]["content"], "OK");
        assert_eq!(body["usage"]["prompt_tokens_details"]["cached_tokens"], 3);
    }
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 1);
    let audit = &audits[0];
    assert_eq!(audit.status, "completed");
    assert_eq!(audit.requested_model.as_deref(), Some("public-alias"));
    assert_eq!(audit.resolved_model.as_deref(), Some("qwen-synthetic"));
    assert_eq!(audit.prompt_tokens, Some(8));
    assert_eq!(audit.completion_tokens, Some(2));
    assert_eq!(audit.cache_read_input_tokens, Some(3));
    assert_eq!(audit.provider_account_id.as_deref(), Some("qwen-pipeline"));
    assert_eq!(
        audit.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
    let states = fixture
        .local
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(states
        .iter()
        .any(|s| s.provider_credential_ref.as_deref() == Some("exact")
            && s.model == "qwen-synthetic"
            && s.status == "active"));
    assert!(!states
        .iter()
        .any(|s| s.provider_credential_ref.as_deref() == Some("unrelated")));
    fixture.finish().await;
    server.stop().await;
}
#[path = "qwen_pipeline_cancellation_tests.rs"]
mod cancellations;
#[path = "qwen_pipeline_response_tests.rs"]
mod responses;
#[path = "qwen_pipeline_stream_tests.rs"]
mod streams;
