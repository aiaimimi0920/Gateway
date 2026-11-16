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
mod auth_tests;
mod boundaries;
mod failures;
mod fixture;
mod management;
#[path = "../dim_oauth/pipeline_tests/native.rs"]
mod native;
mod peer;
mod probe;
mod probe_management;
#[path = "../../upstream/client/vertex_express_pipeline_tool_wire.rs"]
mod public_wire;

#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
use fixture::Fixture;
const MODEL: &str = "public-sap_aicore";
const UPSTREAM_MODEL: &str = "deployment1";
const KEY: &str = super::test_support::KEY;
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
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some(UPSTREAM_MODEL));
    assert_eq!(
        row.provider_account_id.as_deref(),
        Some("sap_aicore-public")
    );
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
    let saved = serde_json::to_string(row).unwrap();
    assert!(!saved.contains(KEY) && !saved.contains("caller-secret"));
}
fn assert_wire(peer: &peer::Peer, count: usize, _endpoint: &str) {
    assert_eq!(peer.oauth.lock().unwrap().len(), 1);
    let calls = peer.calls.lock().unwrap();
    assert_eq!(calls.len(), count, "no replay");
    for (path, headers, raw) in calls.iter() {
        assert_eq!(
            path,
            "/v2/inference/deployments/deployment1/chat/completions?api-version=2024-10-21"
        );
        assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
        assert_eq!(headers["AI-Resource-Group"], "group-a");
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
        assert!(raw.get("model").is_none());
        assert!(raw.get("sap_api_version").is_none());
        assert!(!raw.to_string().contains(KEY));
    }
}
#[tokio::test]
async fn sap_aicore_public_text_json_sse_and_account_billing() {
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
async fn sap_aicore_public_tools_json_sse_two_turns() {
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
