//! Real ordinary-Qwen SSE through public auth, exact routing, translation and durable finalization.
use super::*;
#[path = "qwen_pipeline_stream_support.rs"]
mod stream_support;
use stream_support::*;

#[test]
fn qwen_pipeline_stream_chat_and_responses_keep_terminal_usage_and_utf8() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        let server = wire::Server::stream(
            format!(
                "{}{}{}data: [DONE]\n\n",
                delta("你好"),
                finish("stop"),
                usage()
            ),
            false,
        )
        .await;
        let fixture = Fixture::new(&server).await;
        wire::within(&server.origin, async {
            let response = open(&fixture, endpoint).await;
            assert_eq!(response.status(), StatusCode::OK);
            let (body, failed) = drain(response.into_body()).await;
            assert!(!failed, "{body}");
            assert!(body.contains("你好"), "{body}");
            if endpoint == "responses" {
                assert!(body.contains("response.completed"), "{body}");
                assert!(body.contains("\"cached_tokens\":3"), "{body}");
            } else {
                assert!(body.contains("[DONE]"), "{body}");
                assert!(body.contains("\"finish_reason\":\"stop\""), "{body}");
            }
            let row = audit(&fixture).await;
            assert_eq!(row.status, "completed");
            assert_eq!(row.total_tokens, Some(10));
            assert_eq!(row.cache_read_input_tokens, Some(3));
            assert_identity(&server, &row);
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    }
}
#[test]
fn qwen_pipeline_stream_truncation_and_late_errors_cannot_complete() {
    run(rejects);
}
async fn rejects() {
    let valid = format!(
        "{}{}{}data: [DONE]\n\n",
        delta("VISIBLE"),
        finish("stop"),
        usage()
    );
    for endpoint in ["chat/completions", "responses"] {
        for tail in [
            delta("VISIBLE"),
            format!("{}data: [DONE]\n\n", delta("VISIBLE")),
            format!(
                "{}{}",
                valid,
                frame(json!({"error":{"message":"private-upstream-detail"}}))
            ),
            format!("{}data: {{\"unfinished\":", valid),
        ] {
            let server = wire::Server::stream(tail, false).await;
            let fixture = Fixture::new(&server).await;
            wire::within(&server.origin, async {
                let response = open(&fixture, endpoint).await;
                let (body, transport_failed) = drain(response.into_body()).await;
                assert!(body.contains("VISIBLE"), "Valid deltas lost: {body}");
                assert!(!body.contains("private-upstream-detail"), "{body}");
                assert!(!body.contains("response.completed"), "{body}");
                assert!(
                    transport_failed || body.contains("error"),
                    "Missing failure: {body}"
                );
                let row = audit(&fixture).await;
                assert_ne!(row.status, "completed", "{body}");
                assert!(row.total_tokens.is_none());
                assert!(!row
                    .error_summary
                    .as_deref()
                    .unwrap_or_default()
                    .contains("private-upstream-detail"));
                assert_identity(&server, &row);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    }
}
#[test]
fn qwen_pipeline_stream_drop_before_eof_cancels_without_success_or_usage() {
    run(cancel);
}
async fn cancel() {
    for endpoint in ["chat/completions", "responses"] {
        let server = wire::Server::stream(
            format!(
                "{}{}{}data: [DONE]\n\n",
                delta("VISIBLE"),
                finish("stop"),
                usage()
            ),
            true,
        )
        .await;
        let fixture = Fixture::new(&server).await;
        wire::within(&server.origin, async {
            let response = open(&fixture, endpoint).await;
            let mut body = response.into_body();
            let mut seen = String::new();
            while !seen.contains("VISIBLE") {
                let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
                seen.push_str(std::str::from_utf8(&frame).unwrap());
            }
            // Drive the lazy tail, but not to the client's five-second transport deadline.
            let outcome = tokio::time::timeout(Duration::from_millis(150), async {
                while let Some(frame) = body.frame().await {
                    if let Ok(frame) = frame {
                        if let Ok(bytes) = frame.into_data() {
                            seen.push_str(std::str::from_utf8(&bytes).unwrap());
                        }
                    }
                }
            })
            .await;
            assert!(outcome.is_err(), "Held HTTP response unexpectedly ended");
            assert!(
                !seen.contains("response.completed"),
                "Premature success: {seen}"
            );
            assert!(!seen.contains("[DONE]"), "Premature DONE: {seen}");
            drop(body);
            let row = audit(&fixture).await;
            assert_eq!(row.status, "cancelled");
            assert!(row.total_tokens.is_none());
            assert_identity(&server, &row);
            let states = fixture
                .local
                .list_model_states(db::CredentialModelStateFilters::default())
                .await
                .unwrap();
            assert!(
                states.iter().all(|s| s.failure_count == 0),
                "Caller cancellation poisoned model health"
            );
            tokio::time::timeout(Duration::from_secs(3), async {
                while server.closed() == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    }
}
#[test]
fn qwen_pipeline_stream_first_error_is_fixed_and_not_retried() {
    run(first_error);
}
async fn first_error() {
    for endpoint in ["chat/completions", "responses"] {
        let server = wire::Server::stream(
            frame(json!({"error":{"message":"private-upstream-detail"}})),
            false,
        )
        .await;
        let fixture = Fixture::new(&server).await;
        wire::within(&server.origin, async {
            let response = open(&fixture, endpoint).await;
            let (body, failed) = drain(response.into_body()).await;
            assert!(failed || body.contains("error"), "{body}");
            assert!(!body.contains("private-upstream-detail"));
            let row = audit(&fixture).await;
            assert_eq!(row.status, "failed");
            assert!(!row
                .error_summary
                .as_deref()
                .unwrap_or_default()
                .contains("private-upstream-detail"));
            assert_identity(&server, &row);
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    }
}
#[test]
fn qwen_pipeline_stream_tools_length_filter_keep_complete_output() {
    run(terminals);
}
async fn terminals() {
    for endpoint in ["chat/completions", "responses"] {
        for reason in ["tool_calls", "length", "content_filter"] {
            let ending = if reason == "tool_calls" {
                frame(
                    json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function",
                    "function":{"name":"lookup","arguments":"{\"q\":\"x\"}"}}]},"finish_reason":reason}]}),
                )
            } else {
                format!("{}{}", delta("OK"), finish(reason))
            };
            let server = wire::Server::stream(
                format!("{ending}{}data: [DONE]\n\n: final ping\n\n", usage()),
                false,
            )
            .await;
            let fixture = Fixture::new(&server).await;
            wire::within(&server.origin, async {
                let response = open(&fixture, endpoint).await;
                let (body, failed) = drain(response.into_body()).await;
                assert!(!failed, "{body}");
                if reason == "tool_calls" {
                    assert!(body.contains("lookup"), "{body}");
                }
                if endpoint == "responses" {
                    assert!(
                        body.contains(if reason == "tool_calls" {
                            "response.completed"
                        } else {
                            "response.incomplete"
                        }),
                        "{body}"
                    );
                } else {
                    assert!(
                        body.contains(&format!("\"finish_reason\":\"{reason}\"")),
                        "{body}"
                    );
                }
                let row = audit(&fixture).await;
                assert_eq!(row.status, "completed");
                assert_eq!(row.total_tokens, Some(10));
                assert_eq!(row.cache_read_input_tokens, Some(3));
                assert_identity(&server, &row);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    }
}
