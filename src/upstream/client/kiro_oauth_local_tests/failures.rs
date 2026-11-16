//! Real TCP errors and caller stream drop release the original local reservation without replay.
use super::*;
use futures::StreamExt;

#[test]
fn kiro_oauth_local_tcp_http_and_late_stream_errors_refund_without_replay_or_secret_leak() {
    run(120, failures);
}
async fn failures() {
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            let late = [
                wire::event_frame("VISIBLE"),
                wire::error_frame("private-upstream-detail synthetic-console-access"),
            ]
            .concat();
            for (status, content_type, bytes) in [
                (
                    401,
                    "text/plain",
                    b"private-upstream-detail synthetic-console-access".to_vec(),
                ),
                (302, "text/plain", Vec::new()),
                (200, "application/vnd.amazon.eventstream", late.clone()),
            ] {
                let upstream = wire::Server::start(status, content_type, bytes).await;
                let fixture = Fixture::new(&upstream).await;
                let response = fixture
                    .server
                    .try_open(endpoint, &fixture.token, body(endpoint, stream))
                    .await;
                assert_failed_response(response, status == 200 && stream).await;
                let rows = fixture.audits(1).await;
                fixture.assert_identity(&rows, "failed");
                assert!(rows[0].total_tokens.is_none());
                assert_eq!(remaining(&fixture.server, &fixture.key_id).await, 10);
                assert_wire(&upstream, 1);
                fixture.stop().await;
                upstream.stop().await;
            }
        }
    }
}
async fn assert_failed_response(
    response: Result<rquest::Response, rquest::Error>,
    early_stream_error: bool,
) {
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            // A native stream failure can close HTTP before response headers are committed.
            // The caller then receives a transport failure; audit/refund/no-replay are still required.
            assert!(
                early_stream_error,
                "Unexpected public connection failure: {error}"
            );
            fixture::assert_redacted(&json!(error.to_string()));
            return;
        }
    };
    let status = response.status().as_u16();
    let mut reader = response.bytes_stream();
    let mut bytes = Vec::new();
    let mut body_failed = false;
    while let Some(chunk) = reader.next().await {
        match chunk {
            Ok(chunk) => {
                assert!(bytes.len() + chunk.len() <= 65536);
                bytes.extend_from_slice(&chunk);
            }
            Err(_) => {
                body_failed = true;
                break;
            }
        }
    }
    let text = String::from_utf8(bytes).unwrap();
    fixture::assert_redacted(&json!(text));
    assert!(
        status != 200 || body_failed || text.contains("error"),
        "Failure became public success: {text}"
    );
    assert!(
        !text.contains("response.completed")
            && !text.contains("message_stop")
            && !text.contains("[DONE]"),
        "{text}"
    );
}
#[test]
fn kiro_oauth_local_tcp_stream_drop_cancels_and_refunds_without_failing_account_health() {
    run(90, cancel);
}
async fn cancel() {
    for endpoint in ["chat/completions", "messages", "responses"] {
        let upstream = wire::Server::stream(wire::event_frame("VISIBLE"), true).await;
        let fixture = Fixture::new(&upstream).await;
        let response = fixture
            .server
            .open(endpoint, &fixture.token, body(endpoint, true))
            .await;
        assert_eq!(response.status().as_u16(), 200);
        let mut stream = response.bytes_stream();
        let mut seen = Vec::new();
        while !String::from_utf8_lossy(&seen).contains("VISIBLE") {
            seen.extend_from_slice(&stream.next().await.unwrap().unwrap());
        }
        assert!(!String::from_utf8_lossy(&seen).contains("[DONE]"));
        drop(stream);
        let rows = fixture.audits(1).await;
        fixture.assert_identity(&rows, "cancelled");
        assert!(rows[0].total_tokens.is_none());
        assert_eq!(remaining(&fixture.server, &fixture.key_id).await, 10);
        let states = fixture
            .server
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_model_states(db::CredentialModelStateFilters::default())
            .await
            .unwrap();
        assert!(states.iter().all(|s| s.failure_count == 0));
        tokio::time::timeout(Duration::from_secs(3), async {
            while upstream.closed() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_wire(&upstream, 1);
        fixture.stop().await;
        upstream.stop().await;
    }
}
