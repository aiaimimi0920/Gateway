//! Product errors must not complete or replay under the real public pipeline.
use super::*;

#[test]
fn kiro_api_key_pipeline_empty_eventstream_is_not_a_completed_generation() {
    run(empty);
}
async fn empty() {
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            let server = wire::Server::stream(Vec::new(), false).await;
            let fixture = Fixture::new(&server).await;
            wire::within(&server.origin, async {
                let response = fixture.open(endpoint, body(endpoint, stream), true).await;
                let status = response.status();
                let (text, failed) = drain(response.into_body()).await;
                assert!(
                    status != StatusCode::OK || failed || text.contains("error"),
                    "empty success: {text}"
                );
                assert!(
                    !text.contains("response.completed")
                        && !text.contains("message_stop")
                        && !text.contains("[DONE]"),
                    "{text}"
                );
                let row = fixture.audit().await;
                assert_eq!(row.status, "failed");
                assert!(row.total_tokens.is_none());
                assert_wire(&server);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    }
}
#[test]
fn kiro_api_key_pipeline_http_errors_never_retry_refresh_or_fallback() {
    run(http_errors);
}
async fn http_errors() {
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            for status in [401, 403, 429, 500] {
                let server = wire::Server::start(
                    status,
                    "application/json",
                    br#"{"error":{"message":"private-upstream-detail synthetic-exact-key caller-secret"}}"#.to_vec(),
                )
                .await;
                let fixture = Fixture::new(&server).await;
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, body(endpoint, stream), true).await;
                    let status = response.status();
                    let (text, failed) = drain(response.into_body()).await;
                    assert!(
                        status != StatusCode::OK || failed || text.contains("error"),
                        "{text}"
                    );
                    assert_wire(&server);
                    assert!(
                        !text.contains("private-upstream-detail")
                            && !text.contains("synthetic-exact-key")
                            && !text.contains("caller-secret"),
                        "{text}"
                    );
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "failed");
                    assert!(row.total_tokens.is_none());
                    let saved = serde_json::to_string(&row).unwrap();
                    assert!(
                        !saved.contains("private-upstream-detail")
                            && !saved.contains("synthetic-exact-key")
                            && !saved.contains("caller-secret"),
                        "{saved}"
                    );
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    }
}
#[test]
fn kiro_api_key_pipeline_invalid_public_key_never_reaches_upstream() {
    run(unauthorized);
}
async fn unauthorized() {
    let server = wire::Server::stream(wire::event_frame("unreachable"), false).await;
    let fixture = Fixture::new(&server).await;
    wire::within(&server.origin, async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            let response = fixture.open(endpoint, body(endpoint, false), false).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let (text, _) = drain(response.into_body()).await;
            assert!(!text.contains("synthetic-exact-key"));
        }
    })
    .await;
    assert_eq!(server.attempts(), 0);
    assert!(server.calls.lock().unwrap().is_empty());
    fixture.finish().await;
    server.stop().await;
}
#[test]
fn kiro_api_key_pipeline_unsupported_controls_are_rejected_before_network() {
    run(controls);
}
async fn controls() {
    for (endpoint, patch) in [
        ("responses", json!({"max_output_tokens":32})),
        ("responses", json!({"temperature":0.5})),
        (
            "messages",
            json!({"thinking":{"type":"enabled","budget_tokens":32}}),
        ),
        ("chat/completions", json!({"tool_choice":"required"})),
    ] {
        let server = wire::Server::stream(wire::event_frame("unreachable"), false).await;
        let fixture = Fixture::new(&server).await;
        wire::within(&server.origin, async {
            let mut request = body(endpoint, false);
            request
                .as_object_mut()
                .unwrap()
                .extend(patch.as_object().unwrap().clone());
            let response = fixture.open(endpoint, request, true).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let (text, _) = drain(response.into_body()).await;
            assert!(!text.contains("unreachable"));
            assert_eq!(server.attempts(), 0);
            let row = fixture.audit().await;
            assert_eq!(row.status, "failed");
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    }
}
