//! Invalid HTTP-200 streams must not be converted to successful output or charged as a completed turn.
use super::*;

#[tokio::test]
async fn vertex_express_pipeline_incomplete_or_invalid_is_not_a_billable_success() {
    Box::pin(async {
        let partial =
            event(json!({"candidates":[{"content":{"parts":[{"text":"private-partial"}]}}]}));
        let mut invalid_utf8 = success_body();
        invalid_utf8.extend_from_slice(b"data: \xff\n\n");
        for tail in [
            Vec::new(),
            partial,
            b"data: [DONE]\n\n".to_vec(),
            [success_body(), b"data: {broken}\n\n".to_vec()].concat(),
            [success_body(), b"data: {\"unfinished\":".to_vec()].concat(),
            [
                success_body(),
                event(json!({"error":{"message":format!("private-upstream-detail {KEY}")}})),
            ]
            .concat(),
            invalid_utf8,
        ] {
            for endpoint in ["chat/completions", "messages", "responses"] {
                for stream in [false, true] {
                    let server = wire::Server::start(200, tail.clone()).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin, async {
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert!(
                            !status.is_success() || text.contains("error"),
                            "{endpoint} stream={stream} falsely succeeded: {text}"
                        );
                        assert!(
                            !text.contains("[DONE]")
                                && !text.contains("response.completed")
                                && !text.contains("message_stop"),
                            "{text}"
                        );
                        assert!(
                            !text.contains(KEY)
                                && !text.contains("private-upstream-detail")
                                && !text.contains("private-partial"),
                            "{text}"
                        );
                        let row = fixture.audit().await;
                        assert_eq!(row.status, "failed");
                        assert!(row.total_tokens.is_none());
                        let saved = serde_json::to_string(&row).unwrap();
                        assert!(
                            !saved.contains(KEY) && !saved.contains("private-upstream-detail"),
                            "{saved}"
                        );
                        assert_identity(&row);
                        assert_eq!(fixture.remaining().await, 2);
                        assert_wire(&server);
                    })
                    .await;
                    fixture.finish().await;
                    server.stop().await;
                }
            }
        }
    })
    .await;
}

#[tokio::test]
async fn vertex_express_pipeline_http_errors_refund_without_following_redirects_or_exposing_body() {
    Box::pin(async {
        for status in [307, 401, 403, 429, 503] {
            for stream in [false, true] {
                eprintln!("Express upstream HTTP contract: {status} stream={stream}");
                let server = wire::Server::start(
                    status,
                    format!("private-upstream-detail {KEY}").into_bytes(),
                )
                .await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture
                        .open("chat/completions", body("chat/completions", stream))
                        .await;
                    let response_status = response.status();
                    let text = drain(response.into_body()).await;
                    assert!(!response_status.is_success(), "{text}");
                    assert!(
                        !text.contains(KEY) && !text.contains("private-upstream-detail"),
                        "{text}"
                    );
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "failed");
                    assert!(row.total_tokens.is_none());
                    let saved = serde_json::to_string(&row).unwrap();
                    assert!(
                        !saved.contains(KEY) && !saved.contains("private-upstream-detail"),
                        "{saved}"
                    );
                    assert_identity(&row);
                    assert_eq!(fixture.remaining().await, 2);
                    // The sender never retries itself; the normal pipeline may retry known refusals.
                    assert_wire_attempts(&server, if matches!(status, 429 | 503) { 3 } else { 1 });
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
async fn vertex_express_pipeline_caller_drop_before_aggregate_completion_refunds_and_closes_source()
{
    Box::pin(async {
        for stream in [false, true] {
            let server = wire::Server::stream(
                200,
                event(json!({"candidates":[{"content":{"parts":[{"text":"not-public-yet"}]}}]})),
                true,
            )
            .await;
            let fixture = Box::pin(Fixture::new(&server)).await;
            wire::within(&server.origin, async {
                let mut call =
                    Box::pin(fixture.open("chat/completions", body("chat/completions", stream)));
                assert!(
                    tokio::time::timeout(Duration::from_millis(200), call.as_mut())
                        .await
                        .is_err(),
                    "the aggregate bridge cannot expose a result before upstream EOF"
                );
                assert_wire(&server);
                assert_eq!(
                    fixture.remaining().await,
                    1,
                    "the normal reservation is held while upstream is pending"
                );
                drop(call);
                let row = fixture.audit().await;
                assert_eq!(row.status, "cancelled");
                assert!(row.total_tokens.is_none());
                assert_identity(&row);
                assert_eq!(fixture.remaining().await, 2);
                let states = fixture
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_model_states(db::CredentialModelStateFilters::default())
                    .await
                    .unwrap();
                assert!(states.iter().all(|s| s.failure_count == 0), "{states:?}");
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
    })
    .await;
}

#[tokio::test]
async fn vertex_express_pipeline_actual_token_usage_settles_normal_prepaid_key() {
    Box::pin(async {
        for stream in [false, true] {
            let server = wire::Server::start(200, success_body()).await;
            let fixture = Box::pin(Fixture::new(&server)).await;
            let (status, text) = request(
                &fixture.state,
                &format!(
                    "/v1/internal/gateway/access/keys/{}/balances/adjust",
                    fixture.id
                ),
                support::MANAGEMENT_TOKEN,
                json!({"balanceMode":"token_prepaid","totalTokens":100,"remainingTokens":100}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{text}");
            wire::within(&server.origin, async {
                let response = fixture
                    .open("chat/completions", body("chat/completions", stream))
                    .await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                let row = fixture.audit().await;
                assert_eq!(row.total_tokens, Some(5));
                let balance = AccessBalanceStore::from_state(&fixture.state)
                    .unwrap()
                    .get(&fixture.id)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(balance.remaining_tokens, Some(95));
                assert_eq!(balance.total_tokens, Some(100));
                assert_wire(&server);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
