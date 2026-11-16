//! Public Local Keys exercise the saved catalog, native senders, protocol bridges and settlement.
use crate::{access_balance::AccessBalanceStore, db, http::router::build_router, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
mod cancellation;
mod failures;
mod fixture;
mod native;
mod peer;
#[path = "../../upstream/client/vertex_express_pipeline_tool_wire.rs"]
mod public_wire;
mod signed;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
use fixture::Fixture;
const MODEL: &str = "public-dim";
const ACCESS: &str = "synthetic-dim-public-access";
const FAMILIES: [&str; 4] = ["chat", "responses", "messages", "gemini"];
const ENDPOINTS: [&str; 3] = ["chat/completions", "messages", "responses"];
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
    assert_eq!(row.resolved_model.as_deref(), Some("opaque-model"));
    assert_eq!(row.provider_account_id.as_deref(), Some("dim-public"));
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
    let saved = serde_json::to_string(row).unwrap();
    assert!(!saved.contains(ACCESS) && !saved.contains("caller-secret"));
}
fn assert_wire(server: &peer::Peer, family: &str, count: usize) {
    let calls = server.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        count,
        "never replay an unknown-consumption request"
    );
    for (path, headers, raw) in calls.iter() {
        let expected = match family {
            "chat" => "/v1/chat/completions",
            "responses" => "/v1/responses",
            "messages" => "/v1/messages",
            _ => "/v1beta/models/opaque-model:streamGenerateContent?alt=sse",
        };
        assert_eq!(path, expected);
        assert_eq!(headers["authorization"], format!("Bearer {ACCESS}"));
        for name in [
            "cookie",
            "x-api-key",
            "x-goog-api-key",
            "x-management-token",
        ] {
            assert!(!headers.contains_key(name));
        }
        assert!(raw.to_string().contains("合成问题"));
    }
}
#[tokio::test]
async fn dim_public_four_native_families_three_endpoints_json_and_sse() {
    Box::pin(async {
        for family in FAMILIES {
            for endpoint in ENDPOINTS {
                for stream in [false, true] {
                    let peer = peer::Peer::new(family, false, false, 200).await;
                    let fixture = Box::pin(Fixture::new(&peer, family)).await;
                    super::test_support::origin(peer.origin.clone(), async {
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(
                            status,
                            StatusCode::OK,
                            "{family}/{endpoint}/{stream}: {text}"
                        );
                        assert!(
                            text.contains("合成答案") && !text.contains(ACCESS),
                            "{family}/{endpoint}/{stream}: {text}"
                        );
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
                            (Some(2), Some(3), Some(5)),
                            "{family}/{endpoint}/{stream}"
                        );
                        assert_eq!(fixture.remaining().await, 1);
                        assert_wire(&peer, family, 1);
                    })
                    .await;
                    fixture.finish().await;
                    peer.stop().await;
                }
            }
        }
    })
    .await;
}
#[tokio::test]
async fn dim_public_tools_four_native_families_three_endpoints_json_sse_two_turns() {
    Box::pin(async {
        for family in FAMILIES {
            for endpoint in ENDPOINTS {
                for stream in [false, true] {
                    let peer = peer::Peer::new(family, true, false, 200).await;
                    let fixture = Box::pin(Fixture::new(&peer, family)).await;
                    super::test_support::origin(peer.origin.clone(), async {
                        let response = fixture
                            .open(endpoint, public_wire::request_body(endpoint, stream))
                            .await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(
                            status,
                            StatusCode::OK,
                            "{family}/{endpoint}/{stream}: {text}"
                        );
                        let calls = public_wire::calls(&text, endpoint, stream);
                        assert_eq!(calls.len(), 2, "{family}/{endpoint}/{stream}: {text}");
                        assert_ne!(calls[0].id, calls[1].id);
                        for call in &calls {
                            assert!(!call.id.is_empty());
                            assert_eq!(call.name, "lookup");
                            assert_eq!(call.args, native::args());
                        }
                        assert_eq!(fixture.audits(1).await[0].status, "completed");
                        assert_eq!(fixture.remaining().await, 1);
                        let response = fixture
                            .open(endpoint, public_wire::followup(endpoint, stream, &calls))
                            .await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(
                            status,
                            StatusCode::OK,
                            "{family}/{endpoint}/{stream}: {text}"
                        );
                        assert!(text.contains("合成答案"), "{text}");
                        for row in fixture.audits(2).await {
                            assert_identity(&row);
                            assert_eq!(row.status, "completed");
                            assert_eq!(row.total_tokens, Some(5));
                        }
                        assert_eq!(fixture.remaining().await, 0);
                        assert_wire(&peer, family, 2);
                        native::assert_tools(&peer.calls.lock().unwrap(), family, &calls);
                    })
                    .await;
                    fixture.finish().await;
                    peer.stop().await;
                }
            }
        }
    })
    .await;
}
