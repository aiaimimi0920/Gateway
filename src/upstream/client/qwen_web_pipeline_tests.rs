//! Public authentication, exact credential routing, streaming finalization and real local audit owner.
use crate::{
    db, http::router::build_router, local_runtime::LocalRuntime,
    upstream::qwen::web_session::test_support as wire,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "qwen_web_pipeline_tests/browser_host.rs"]
mod browser_host;
#[path = "qwen_web_pipeline_tests/prepaid.rs"]
mod prepaid;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
#[path = "qwen_web_pipeline_tests/tcp.rs"]
mod tcp;

fn document() -> Value {
    let provider = json!({"id":"qwen-pipeline","preset":"qwen-web-chat","base_url":"https://chat.qwen.ai/api/v1",
    "explicit_credentials_only":true,"supported_models":["public-alias"],"model_map":{"public-alias":"qwen3.7-plus"},
    "call_flow":{"special_adapter":"qwen_web","fingerprint_adapter":"session"},
    "credentials":[
        {"id":"other","api_key":"synthetic-other","supported_models":["other-model"]},
        {"id":"exact","api_key":"synthetic-exact","headers":{"Cookie":"token=synthetic-exact; cf_clearance=synthetic-clearance","User-Agent":"exact-UA"},"supported_models":["public-alias"]}
    ]});
    let mut backup = provider.clone();
    backup["id"] = json!("qwen-backup");
    backup["credentials"][0]["id"] = json!("backup-other");
    backup["credentials"][1]["id"] = json!("backup-exact");
    backup["credentials"][1]["api_key"] = json!("synthetic-backup");
    backup["credentials"][1]["headers"]["Cookie"] = json!("token=synthetic-backup");
    json!({"providers":[provider,backup],"model_routes":[{"pattern":"public-alias","provider_ids":["qwen-pipeline","qwen-backup"]}]})
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
                    tokio::time::timeout(Duration::from_secs(45), test())
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
        let path = std::env::temp_dir().join(format!("qwen-pipeline-{}", uuid::Uuid::new_v4()));
        let local = LocalRuntime::open(&path).await.unwrap();
        let state = Arc::get_mut(&mut console.state).unwrap();
        state.local_runtime = Some(local.clone());
        state.upstream_client = super::UpstreamClient::new(5);
        state.upstream_client.http = server.client();
        Self { console, local }
    }
    async fn request(&self, server: &wire::Server, endpoint: &str, body: Value) -> Response {
        let request = Request::post(format!("/v1/{endpoint}"))
            .header("authorization", "Bearer public-api-contract-secret")
            .header("cookie", "foreign=caller-secret")
            .header("user-agent", "caller-UA")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        wire::within(
            &server.origin,
            build_router(self.console.state.clone()).oneshot(request),
        )
        .await
        .unwrap()
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
                        .all(|r| !matches!(r.status.as_str(), "started" | "running"))
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
        json!({"model":"public-alias","input":"合成问题","stream":stream})
    } else {
        json!({"model":"public-alias","messages":[{"role":"user","content":"合成问题"}],"stream":stream})
    }
}
fn assert_wire(server: &wire::Server) {
    let calls = server.calls();
    assert_eq!(
        calls.len(),
        2,
        "One create + one completion; no replay/account/channel fallback"
    );
    for call in calls {
        assert_eq!(
            call.headers["cookie"],
            "token=synthetic-exact; cf_clearance=synthetic-clearance"
        );
        assert_eq!(call.headers["user-agent"], "exact-UA");
        assert_eq!(call.headers["authorization"], "Bearer synthetic-exact");
    }
    assert_eq!(server.calls()[1].body["model"], "qwen3.7-plus");
}
#[test]
fn qwen_web_public_chat_responses_buffered_stream_exact_account_and_usage() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            for reported in [false, true] {
                let mut reply = wire::Reply::default();
                if !reported {
                    reply.chunks[1] = bytes::Bytes::from_static(b"data: {\"done\":true}\n\n");
                }
                let server = wire::Server::start(reply).await;
                let fixture = Fixture::new(&server).await;
                let response = fixture
                    .request(&server, endpoint, body(endpoint, stream))
                    .await;
                let status = response.status();
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                let text = String::from_utf8_lossy(&bytes);
                assert_eq!(status, StatusCode::OK, "{endpoint} stream={stream}: {text}");
                assert!(text.contains("合成答案"), "{text}");
                if stream {
                    assert!(text.contains(if endpoint == "responses" {
                        "response.completed"
                    } else {
                        "[DONE]"
                    }));
                } else {
                    let value: Value = serde_json::from_slice(&bytes).unwrap();
                    if reported {
                        assert_eq!(value["usage"]["total_tokens"], 5, "{value}");
                    } else {
                        assert!(value["usage"].is_null(), "{value}");
                    }
                }
                assert_wire(&server);
                let rows = fixture.audits().await;
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].status, "completed");
                assert_eq!(rows[0].resolved_model.as_deref(), Some("qwen3.7-plus"));
                assert_eq!(
                    rows[0].route_trace.as_ref().unwrap()["realCredentialRef"],
                    "exact"
                );
                if reported {
                    assert_eq!(rows[0].total_tokens, Some(5));
                } else {
                    assert!(
                        rows[0].prompt_tokens.is_none()
                            && rows[0].completion_tokens.is_none()
                            && rows[0].total_tokens.is_none()
                    );
                }
                fixture.finish().await;
                server.finish().await;
            }
        }
    }
}
#[test]
fn qwen_web_public_buffered_and_partial_stream_failures_are_not_completed() {
    run(failures);
}
async fn failures() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let mut reply = wire::Reply::default();
            reply.chunks.pop();
            let server = wire::Server::start(reply).await;
            let fixture = Fixture::new(&server).await;
            let response = fixture
                .request(&server, endpoint, body(endpoint, stream))
                .await;
            if !stream {
                assert_ne!(response.status(), StatusCode::OK);
            }
            let bytes = response.into_body().collect().await;
            if let Ok(bytes) = bytes {
                let text = String::from_utf8_lossy(&bytes.to_bytes()).into_owned();
                assert!(
                    !text.contains("response.completed") && !text.contains("[DONE]"),
                    "{text}"
                );
            }
            assert_wire(&server);
            let rows = fixture.audits().await;
            assert_eq!(rows.len(), 1);
            assert_ne!(rows[0].status, "completed");
            fixture.finish().await;
            server.finish().await;
        }
    }
}
#[test]
fn qwen_web_public_stream_caller_drop_settles_once_and_closes_body() {
    run(cancelled);
}
async fn cancelled() {
    let mut reply = wire::Reply::default();
    reply.chunks.pop();
    reply.hold = true;
    let server = wire::Server::start(reply).await;
    let fixture = Fixture::new(&server).await;
    let response = fixture
        .request(&server, "responses", body("responses", true))
        .await;
    let mut body = response.into_body();
    let mut seen = String::new();
    while !seen.contains("合成答案") {
        let frame = body.frame().await.unwrap().unwrap();
        if let Ok(bytes) = frame.into_data() {
            seen.push_str(&String::from_utf8_lossy(&bytes));
        }
        assert!(seen.len() < 16384);
    }
    drop(body);
    server.wait_closed().await;
    let rows = fixture.audits().await;
    assert_eq!(rows.len(), 1);
    assert_ne!(rows[0].status, "completed");
    assert_wire(&server);
    fixture.finish().await;
    server.finish().await;
}
