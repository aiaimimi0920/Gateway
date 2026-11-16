//! Damaged or unfinished output cannot acquire a success envelope or settle a completed message.
use super::*;
#[tokio::test]
async fn vertex_oauth_pipeline_invalid_stream_is_not_billable_success() {
    Box::pin(async {
        for raw in [
            Vec::new(),
            event(json!({"candidates":[{"content":{"parts":[{"text":"private-partial"}]}}]})),
            b"data: [DONE]\n\n".to_vec(),
            [success_body(), b"data: {broken}\n\n".to_vec()].concat(),
            [success_body(), b"data: {\"unfinished\":".to_vec()].concat(),
            [success_body(), event(json!({"error":{"message":format!("{PRIVATE} {ACCESS}")}}))].concat(),
            [success_body(), b"data: \xff\n\n".to_vec()].concat(),
            [success_body(), event(candidate("STOP"))].concat(),
            event(json!({"candidates":[{"index":0,"finishReason":"STOP"},{"index":1,"finishReason":"STOP"}]})),
            event(candidate("FINISH_REASON_UNSPECIFIED")),
            event(candidate("unknown-finish")),
            event(json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"incomplete","partialArgs":[{}]}}]},"finishReason":"STOP"}]})),
        ] {
            for endpoint in ["chat/completions", "messages", "responses"] {
                for stream in [false, true] {
                    let server = wire::Server::start(200, raw.clone()).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin, async {
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert!(!status.is_success() || text.contains("error"), "{endpoint} stream={stream} falsely succeeded: {text}");
                        assert!(!text.contains("[DONE]") && !text.contains("response.completed") && !text.contains("message_stop"), "{text}");
                        assert!(!text.contains(ACCESS) && !text.contains(PRIVATE) && !text.contains("private-partial"), "{text}");
                        let row = fixture.audit().await;
                        assert_eq!(row.status, "failed");
                        assert!(row.total_tokens.is_none());
                        let saved = serde_json::to_string(&row).unwrap();
                        assert!(!saved.contains(ACCESS) && !saved.contains(PRIVATE), "{saved}");
                        assert_identity(&row);
                        assert_eq!(fixture.remaining().await, 2);
                        assert_wire(&server, 1);
                    }).await;
                    fixture.finish().await;
                    server.stop().await;
                }
            }
        }
    }).await;
}
#[tokio::test]
async fn vertex_oauth_pipeline_http_error_is_classified_redacted_and_refunded() {
    Box::pin(async {
        for status in [307, 401, 403, 429, 503] {
            for stream in [false, true] {
                let server =
                    wire::Server::start(status, format!("{PRIVATE} {ACCESS}").into_bytes()).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture
                        .open("chat/completions", body("chat/completions", stream))
                        .await;
                    let status_code = response.status();
                    let text = drain(response.into_body()).await;
                    assert!(!status_code.is_success(), "{text}");
                    assert!(!text.contains(PRIVATE) && !text.contains(ACCESS), "{text}");
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "failed");
                    assert!(row.total_tokens.is_none());
                    let saved = serde_json::to_string(&row).unwrap();
                    assert!(
                        !saved.contains(PRIVATE) && !saved.contains(ACCESS),
                        "{saved}"
                    );
                    assert_identity(&row);
                    assert_eq!(fixture.remaining().await, 2);
                    // Existing pipeline policy may retry known refusals, never follow the synthetic Location.
                    assert_wire(&server, if matches!(status, 429 | 503) { 3 } else { 1 });
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}
#[tokio::test]
async fn vertex_oauth_pipeline_body_timeout_is_not_replayed_as_unstarted_generation() {
    Box::pin(async {
        let server = wire::Server::stream(200, success_body(), true).await;
        let fixture = Box::pin(Fixture::new(&server)).await;
        wire::within(&server.origin, async {
            let response = fixture
                .open("chat/completions", body("chat/completions", false))
                .await;
            assert!(
                !response.status().is_success(),
                "EOF must be verified even after finish"
            );
            let text = drain(response.into_body()).await;
            assert!(!text.contains("合成答案"), "{text}");
            let row = fixture.audit().await;
            assert_eq!(row.status, "failed");
            assert_identity(&row);
            assert_eq!(fixture.remaining().await, 2);
            assert_wire(&server, 1);
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
