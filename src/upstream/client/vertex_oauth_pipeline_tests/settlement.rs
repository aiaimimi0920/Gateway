//! Further balance and cancellation contracts are kept separate from stream integrity cases.
use super::*;
#[tokio::test]
async fn vertex_oauth_pipeline_token_key_preserves_reported_total_zero_and_cache() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                for total in [0, 17] {
                    let mut raw = candidate("STOP");
                    raw["usageMetadata"] = json!({"promptTokenCount":9,"candidatesTokenCount":3,"totalTokenCount":total,
                        "cachedContentTokenCount":2,"thoughtsTokenCount":5});
                    let server = wire::Server::start(200, event(raw)).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    let (status, text) = fixture.token_balance().await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    wire::within(&server.origin, async {
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::OK, "{endpoint} stream={stream}: {text}");
                        let row = fixture.audit().await;
                        assert_eq!(row.status, "completed");
                        assert_eq!((row.prompt_tokens, row.completion_tokens, row.total_tokens), (Some(9),Some(3),Some(total)), "{endpoint} stream={stream} total={total}");
                        assert_eq!(row.cache_read_input_tokens, Some(2));
                        assert_identity(&row);
                        let balance = AccessBalanceStore::from_state(&fixture.state).unwrap().get(&fixture.id).await.unwrap().unwrap();
                        assert_eq!(balance.remaining_tokens, Some(i64::from(100-total)));
                        assert_eq!(balance.total_tokens, Some(100));
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
async fn vertex_oauth_pipeline_missing_total_is_unknown_not_a_synthesized_zero_bill() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let mut raw = candidate("STOP");
                raw["usageMetadata"] = json!({"promptTokenCount":9});
                let server = wire::Server::start(200, event(raw)).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                let (status, text) = fixture.token_balance().await;
                assert_eq!(status, StatusCode::OK, "{text}");
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, body(endpoint, stream)).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{endpoint} stream={stream}: {text}");
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "completed");
                    assert!(
                        row.total_tokens.is_none(),
                        "{endpoint} stream={stream}: {row:?}"
                    );
                    let balance = AccessBalanceStore::from_state(&fixture.state)
                        .unwrap()
                        .get(&fixture.id)
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(
                        balance.remaining_tokens.unwrap() < 100,
                        "Unknown usage must retain the normal reservation policy"
                    );
                    assert_identity(&row);
                    assert_wire(&server, 1);
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
async fn vertex_oauth_pipeline_caller_drop_refunds_and_closes_source_for_three_protocols() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::stream(
                    200,
                    event(
                        json!({"candidates":[{"content":{"parts":[{"text":"private-partial"}]}}]}),
                    ),
                    true,
                )
                .await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let mut call = Box::pin(fixture.open(endpoint, body(endpoint, stream)));
                    assert!(
                        tokio::time::timeout(Duration::from_millis(200), call.as_mut())
                            .await
                            .is_err()
                    );
                    assert_wire(&server, 1);
                    assert_eq!(fixture.remaining().await, 1);
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
        }
    })
    .await;
}
