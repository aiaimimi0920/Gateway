//! Real public auth/route/send/finalize over a scoped local wire, with exact session ownership.
use crate::upstream::perplexity_web::test_support as wire;
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

fn document() -> Value {
    let provider = json!({"id":"perplexity-pipeline","preset":"perplexity-web","base_url":"https://www.perplexity.ai",
    "explicit_credentials_only":true,"supported_models":["public-alias"],"model_map":{"public-alias":"turbo"},
    "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"perplexity_web"},
    "credentials":[
        {"id":"unrelated","api_key":"synthetic-unrelated","headers":{"User-Agent":"unrelated-UA"},"session_auth":{"transport":"cookie","primaryCookieName":"__Secure-next-auth.session-token"},"supported_models":["another-model"]},
        {"id":"exact","api_key":"synthetic-exact","headers":{"User-Agent":"exact-UA","Cookie":"__Secure-next-auth.session-token=synthetic-exact; cf_clearance=synthetic-clearance"},"session_auth":{"transport":"cookie","primaryCookieName":"__Secure-next-auth.session-token"},"supported_models":["public-alias"]}
    ]});
    let mut backup = provider.clone();
    backup["id"] = json!("perplexity-backup");
    backup["credentials"][0]["id"] = json!("backup-unrelated");
    backup["credentials"][1]["id"] = json!("backup-exact");
    backup["credentials"][1]["api_key"] = json!("synthetic-backup");
    backup["credentials"][1]["headers"]["Cookie"] =
        json!("__Secure-next-auth.session-token=synthetic-backup");
    // Keep a usable backup, but do not let weighted routing randomize the asserted principal.
    json!({"providers":[provider,backup],"model_routes":[
        {"pattern":"public-alias","provider_ids":["perplexity-pipeline"],"priority":20},
        {"pattern":"public-alias","provider_ids":["perplexity-backup"],"priority":10}]})
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
            std::env::temp_dir().join(format!("perplexity-pipeline-{}", uuid::Uuid::new_v4()));
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
        body: Value,
        authorized: bool,
    ) -> (StatusCode, Vec<u8>) {
        let request = Request::post(format!("/v1/{endpoint}"))
            .header(
                "authorization",
                if authorized {
                    "Bearer public-api-contract-secret"
                } else {
                    "Bearer invalid-key"
                },
            )
            .header("cookie", "foreign=synthetic-caller-cookie")
            .header("user-agent", "caller-UA")
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
        let bytes = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, bytes)
    }
    async fn audits(&self) -> Vec<db::GatewayRequestAuditView> {
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
                        .all(|row| !matches!(row.status.as_str(), "started" | "running"))
                {
                    return rows;
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
fn body(endpoint: &str, stream: bool) -> Value {
    if endpoint == "responses" {
        json!({"model":"public-alias","input":"合成问题","stream":stream})
    } else {
        json!({"model":"public-alias","messages":[{"role":"user","content":"合成问题"}],"stream":stream})
    }
}
fn assert_wire(server: &wire::Server) {
    let calls = server.calls();
    assert_eq!(
        calls.len(),
        1,
        "No retry or channel fallback, even when a backup candidate exists"
    );
    assert_eq!(server.attempts(), 1);
    assert_eq!(calls[0].path, "/rest/sse/perplexity_ask");
    assert_eq!(
        calls[0].headers["cookie"],
        "__Secure-next-auth.session-token=synthetic-exact; cf_clearance=synthetic-clearance"
    );
    assert_eq!(calls[0].headers["user-agent"], "exact-UA");
    assert!(!calls[0].headers.contains_key("authorization"));
    assert_eq!(calls[0].body["query_str"], "合成问题");
    assert_eq!(calls[0].body["params"]["model_preference"], "turbo");
}
#[test]
fn perplexity_web_pipeline_chat_and_responses_buffered_and_aggregated_stream() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let server = wire::Server::start(wire::completed(), 200, true).await;
            let fixture = Fixture::new(&server).await;
            let (status, bytes) = fixture
                .request(&server, endpoint, body(endpoint, stream), true)
                .await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
            assert_wire(&server);
            if stream {
                let text = String::from_utf8(bytes).unwrap();
                assert!(
                    text.contains("合成答案") && text.contains("gateway_sources"),
                    "{text}"
                );
                assert!(text.contains(if endpoint == "responses" {
                    "response.completed"
                } else {
                    "[DONE]"
                }));
                assert!(!text.contains("synthetic-private-followup"));
            } else {
                let value: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(value["gateway_sources"][0]["name"], "来源", "{value}");
                assert!(
                    value["usage"].is_null(),
                    "Unknown usage must remain unknown: {value}"
                );
                if endpoint == "responses" {
                    assert_eq!(value["output"][0]["content"][0]["text"], "合成答案");
                } else {
                    assert_eq!(value["choices"][0]["message"]["content"], "合成答案");
                }
            }
            let rows = fixture.audits().await;
            assert_eq!(rows.len(), 1);
            let row = &rows[0];
            assert_eq!(row.status, "completed");
            assert_eq!(row.requested_model.as_deref(), Some("public-alias"));
            assert_eq!(row.resolved_model.as_deref(), Some("turbo"));
            assert_eq!(
                row.route_trace.as_ref().unwrap()["realCredentialRef"],
                "exact"
            );
            assert!(
                row.prompt_tokens.is_none()
                    && row.completion_tokens.is_none()
                    && row.total_tokens.is_none()
            );
            fixture.finish().await;
        }
    }
}
#[path = "perplexity_web_pipeline_failure_tests.rs"]
mod failures;
#[path = "perplexity_web_live_pipeline_tests.rs"]
mod live;
#[path = "perplexity_web_workflow_pipeline_tests.rs"]
mod workflow;
