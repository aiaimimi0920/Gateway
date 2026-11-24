//! 普通预付 Key 覆盖公共三入口，不以单独 sender 测试代替产品链路。
use crate::{access_balance::AccessBalanceStore, db, http::router::build_router, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
mod boundaries;
mod catalog_restart;
mod chat_images;
mod fixture;
mod management;
#[path = "../dim_oauth/pipeline_tests/native.rs"]
mod native;
mod native_json;
mod native_messages;
mod native_messages_extended;
mod native_probe;
mod native_request;
mod native_responses;
mod native_responses_extended;
#[path = "../dim_oauth/pipeline_tests/peer.rs"]
mod peer;
mod probe;
mod probe_management;
#[path = "../../upstream/client/vertex_express_pipeline_tool_wire.rs"]
mod public_wire;
mod response_contract;
mod server;
mod server_probe_audit;
mod server_scheduler;
mod stream_failures;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
mod systemone;
mod systemone_access;
mod systemone_boundaries;
mod systemone_probe;
mod zdr;
use fixture::Fixture;
const MODEL: &str = "public-commandcode";
const UPSTREAM_MODEL: &str = "o3-synthetic-chat";
const KEY: &str = "cpk-synthetic-commandcode";
const ENDPOINTS: [&str; 3] = ["chat/completions", "messages", "responses"];
fn family(_endpoint: &str) -> &'static str {
    "chat"
}
fn body(endpoint: &str, stream: bool) -> Value {
    match endpoint {
        "responses" => json!({"model":MODEL,"input":"合成问题","stream":stream,"store":false}),
        "messages" => {
            json!({"model":MODEL,"max_tokens":32,"messages":[{"role":"user","content":"合成问题"}],"stream":stream})
        }
        _ => {
            json!({"model":MODEL,"messages":[{"role":"user","content":"合成问题"}],"stream":stream})
        }
    }
}
async fn drain(body: Body) -> String {
    let bytes = tokio::time::timeout(Duration::from_secs(10), body.collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}
fn assert_identity(row: &db::GatewayRequestAuditView) {
    assert_identity_among(row, &["exact"]);
}
fn assert_identity_among(row: &db::GatewayRequestAuditView, eligible: &[&str]) {
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some(UPSTREAM_MODEL));
    assert_eq!(
        row.provider_account_id.as_deref(),
        Some("commandcode-public")
    );
    assert!(eligible.contains(
        &row.route_trace.as_ref().unwrap()["realCredentialRef"]
            .as_str()
            .unwrap()
    ));
    let saved = serde_json::to_string(row).unwrap();
    assert!(!saved.contains(KEY) && !saved.contains("caller-secret"));
}
fn assert_wire(peer: &peer::Peer, count: usize, _endpoint: &str) {
    let calls = peer.calls.lock().unwrap();
    assert_eq!(calls.len(), count, "no replay");
    for (path, headers, raw) in calls.iter() {
        assert_eq!(path, "/provider/v1/chat/completions");
        assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
        assert_eq!(headers["user-agent"], "NeuroGateway/CommandCode");
        for name in [
            "cookie",
            "x-api-key",
            "x-goog-api-key",
            "x-management-token",
            "originator",
            "chatgpt-account-id",
        ] {
            assert!(!headers.contains_key(name), "{name}");
        }
        assert_eq!(raw["model"], UPSTREAM_MODEL);
        assert!(!raw.to_string().contains(KEY));
    }
}
#[tokio::test]
async fn commandcode_public_text_json_sse_and_account_billing() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let peer = peer::Peer::new(family(endpoint), false, false, 200).await;
                let fixture = Box::pin(Fixture::new(&peer)).await;
                super::test_support::origin(peer.origin.clone(), async {
                    let response = fixture.open(endpoint, body(endpoint, stream)).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{endpoint}/{stream}: {text}");
                    assert!(text.contains("合成答案"), "{text}");
                    if stream {
                        assert!(
                            text.contains(match endpoint {
                                "messages" => "message_stop",
                                "responses" => "response.completed",
                                _ => "[DONE]",
                            }),
                            "{text}"
                        );
                    }
                    let rows = fixture.audits(1).await;
                    assert_identity(&rows[0]);
                    assert_eq!(rows[0].status, "completed");
                    assert_eq!(
                        (
                            rows[0].prompt_tokens,
                            rows[0].completion_tokens,
                            rows[0].total_tokens
                        ),
                        (Some(2), Some(3), Some(5))
                    );
                    assert_eq!(fixture.remaining().await, 1);
                    assert_wire(&peer, 1, endpoint);
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
#[tokio::test]
async fn commandcode_public_tools_json_sse_two_turns() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let peer = peer::Peer::new(family(endpoint), true, false, 200).await;
                let fixture = Box::pin(Fixture::new(&peer)).await;
                super::test_support::origin(peer.origin.clone(), async {
                    let mut first = public_wire::request_body(endpoint, stream);
                    first["model"] = json!(MODEL);
                    first["tool_choice"] = if endpoint == "messages" {
                        json!({"type":"auto"})
                    } else {
                        json!("auto")
                    };
                    let response = fixture.open(endpoint, first).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{endpoint}/{stream}: {text}");
                    let calls = public_wire::calls(&text, endpoint, stream);
                    assert_eq!(calls.len(), 2, "{text}");
                    assert_ne!(calls[0].id, calls[1].id);
                    for call in &calls {
                        assert!(!call.id.is_empty());
                        assert_eq!(call.name, "lookup");
                        assert_eq!(call.args, native::args());
                    }
                    assert_eq!(fixture.audits(1).await[0].status, "completed");
                    let mut next = public_wire::followup(endpoint, stream, &calls);
                    next["model"] = json!(MODEL);
                    let response = fixture.open(endpoint, next).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{endpoint}/{stream}: {text}");
                    assert!(text.contains("合成答案"));
                    for row in fixture.audits(2).await {
                        assert_identity(&row);
                        assert_eq!(row.status, "completed");
                        assert_eq!(row.total_tokens, Some(5));
                    }
                    assert_eq!(fixture.remaining().await, 0);
                    assert_wire(&peer, 2, endpoint);
                    native::assert_tools(&peer.calls.lock().unwrap(), family(endpoint), &calls);
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
