//! Public OpenAI usage and native audit have intentionally different cache semantics.
use super::*;
use crate::protocol::canonical::TokenUsage;

#[test]
fn cursor_remote_openai_usage_preserves_counts_without_inventing_cache_details() {
    run(|| async {
        for mode in 0..3 {
            let expected = (mode != 0).then_some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cache_read_input_tokens: (mode == 2).then_some(20),
                cache_creation_input_tokens: (mode == 2).then_some(3),
            });
            let supplied = expected.clone();
            let state = app_worker(
                true,
                Runtime::fixture(move |_, hooks| {
                    let usage = supplied.clone();
                    Box::pin(async move {
                        hooks.authorize().await?;
                        hooks.sent()?;
                        Ok(WorkerOutput {
                            text: "native counts".into(),
                            thinking: None,
                            requested_model: "default".into(),
                            usage,
                        })
                    })
                }),
            )
            .await;
            let token = key(&state, "relay", "cursor-remote").await;
            for chat in [false, true] {
                for stream in [false, true] {
                    let mut body = if chat {
                        json!({"model":"cursor-personal","messages":[{"role":"user","content":"hello"}]})
                    } else {
                        json!({"model":"cursor-personal","input":"hello","store":false})
                    };
                    body["stream"] = json!(stream);
                    let endpoint = if chat {
                        "/v1/chat/completions"
                    } else {
                        "/v1/responses"
                    };
                    let (status, raw) = post_to(&state, &token, endpoint, body).await;
                    assert_eq!(status, StatusCode::OK, "{raw}");
                    assert!(!raw.contains("\"prompt_tokens_details\""), "{raw}");
                    assert!(!raw.contains("\"input_tokens_details\""), "{raw}");
                    if !stream {
                        let value: Value = serde_json::from_str(&raw).unwrap();
                        if expected.is_some() {
                            let input = if chat {
                                "prompt_tokens"
                            } else {
                                "input_tokens"
                            };
                            let output = if chat {
                                "completion_tokens"
                            } else {
                                "output_tokens"
                            };
                            assert_eq!(value["usage"][input], 10);
                            assert_eq!(value["usage"][output], 5);
                            assert_eq!(value["usage"]["total_tokens"], 15);
                        } else {
                            assert!(value["usage"].is_null());
                        }
                    }
                }
            }
            let rows = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let rows = state
                        .local_runtime
                        .as_ref()
                        .unwrap()
                        .list_audits(&db::RequestAuditFilters::default())
                        .await
                        .unwrap();
                    if rows.len() == 4 && rows.iter().all(|r| r.completed_at.is_some()) {
                        break rows;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            for row in rows {
                assert_eq!(row.upstream_status, Some(200));
                assert_eq!(row.prompt_tokens, expected.as_ref().map(|_| 10));
                assert_eq!(row.completion_tokens, expected.as_ref().map(|_| 5));
                assert_eq!(row.total_tokens, expected.as_ref().map(|_| 15));
                assert_eq!(row.cache_read_input_tokens, (mode == 2).then_some(20));
                assert_eq!(row.cache_creation_input_tokens, (mode == 2).then_some(3));
                let trace = row.route_trace.unwrap();
                assert_eq!(trace["realCredentialRef"], "cursor-account");
                assert_eq!(trace["routeAttemptCount"], 1);
            }
            state.lifecycle.cursor_remote.join().await;
            state.local_runtime.as_ref().unwrap().close().await;
        }
    });
}
