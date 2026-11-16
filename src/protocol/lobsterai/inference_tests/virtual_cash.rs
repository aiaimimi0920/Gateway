//! Public cash accounting must settle the admitted yd tariff after concrete model resolution.
use super::super::test_support::{Peer, Reply};
use super::*;
#[tokio::test]
async fn lobsterai_virtual_cash_settles_original_quote_and_releases_failed_resolution() {
    Box::pin(async {
        for family in ["openai", "anthropic"] {
            for upstream_sse in [false, true] {
                for success in [false, true] {
                    for endpoint in ENDPOINTS {
                        for stream in [false, true] {
                            let replies = if success {
                                vec![
                                    virtual_models::resolution(family),
                                    cached_reply(family, upstream_sse),
                                ]
                            } else {
                                vec![Reply::raw(403, vec![])]
                            };
                            let peer = Peer::start(replies).await;
                            let fixture = Box::pin(Fixture::with_quota(
                                peer.builder()
                                    .retry(rquest::retry::Policy::never())
                                    .build()
                                    .unwrap(),
                                peer.origin.clone(),
                                "yd-fast",
                                json!([{"modelId":"yd-fast","apiFormat":"openai"}]),
                                true,
                            ))
                            .await;
                            super::super::transport::TEST_ORIGIN
                                .scope(peer.origin.clone(), async {
                                    let mut raw = body(endpoint, stream);
                                    if endpoint == "responses" {
                                        raw["max_output_tokens"] = json!(32);
                                    } else if endpoint == "messages" {
                                        raw.as_object_mut().unwrap().remove("max_tokens");
                                        raw[if upstream_sse {
                                            "max_output_tokens"
                                        } else {
                                            "max_completion_tokens"
                                        }] = json!(32);
                                    } else {
                                        raw["max_tokens"] = json!(32);
                                    }
                                    let response = fixture.open(endpoint, raw).await;
                                    let status = response.status();
                                    let text = drain(response.into_body()).await;
                                    assert_eq!(status == StatusCode::OK, success, "{text}");
                                    if success && family == "anthropic" && endpoint != "messages" {
                                        assert!(
                                            text.contains(if endpoint == "responses" {
                                                "\"input_tokens\":12"
                                            } else {
                                                "\"prompt_tokens\":12"
                                            }),
                                            "{text}"
                                        );
                                    }
                                    if success && family == "anthropic" && endpoint == "messages" {
                                        assert!(
                                            text.contains("\"cache_read_input_tokens\":4"),
                                            "{text}"
                                        );
                                        assert!(
                                            text.contains("\"cache_creation_input_tokens\":6"),
                                            "{text}"
                                        );
                                        assert!(text.contains("\"input_tokens\":2"), "{text}");
                                    }
                                })
                                .await;
                            assert_eq!(fixture.audits(1).await[0].status == "completed", success);
                            let store = AccessBalanceStore::from_state(&fixture.state).unwrap();
                            let ledger = store.cash_ledger(&fixture.id).await.unwrap();
                            assert_eq!(ledger.len(), 1);
                            assert_eq!(
                                ledger[0].status,
                                if success { "settled" } else { "released" }
                            );
                            assert_eq!(ledger[0].amount_micros, Some(if success { 18 } else { 0 }));
                            let balance = store.cash_balance(&fixture.id).await.unwrap().unwrap();
                            assert_eq!(balance.spent_micros, if success { 18 } else { 0 });
                            assert_eq!(balance.reserved_micros, 0);
                            assert_eq!(balance.pending_requests, 0);
                            if success {
                                let quote = ledger[0].settled_quote.as_ref().unwrap();
                                assert_eq!(quote.model, "yd-fast");
                                assert!(!quote.cache_tokens_separate);
                                assert_eq!(ledger[0].usage.as_ref().unwrap().prompt_tokens, 12);
                                assert_eq!(
                                    ledger[0].usage.as_ref().unwrap().cache_read_input_tokens,
                                    Some(4)
                                );
                                if family == "anthropic" {
                                    assert_eq!(
                                        ledger[0]
                                            .usage
                                            .as_ref()
                                            .unwrap()
                                            .cache_creation_input_tokens,
                                        Some(6)
                                    );
                                }
                            }
                            fixture.finish().await;
                            peer.inspect(|calls| {
                                assert_eq!(calls.len(), if success { 2 } else { 1 });
                                if success && family == "anthropic" {
                                    assert_eq!(calls[1].body["max_tokens"], 32);
                                    assert!(calls[1].body.get("max_output_tokens").is_none());
                                    assert!(calls[1].body.get("max_completion_tokens").is_none());
                                }
                            });
                        }
                    }
                }
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_virtual_caller_cancellation_stops_resolution_and_refunds() {
    let peer = Peer::start(vec![Reply::stalled()]).await;
    let fixture = Box::pin(virtual_models::fixture(&peer, "yd-fast")).await;
    let future = super::super::transport::TEST_ORIGIN.scope(
        peer.origin.clone(),
        fixture.open("chat/completions", body("chat/completions", false)),
    );
    let mut pending = Box::pin(future);
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            let mut seen = false;
            peer.inspect(|calls| seen = !calls.is_empty());
            if seen { break; }
            tokio::select! { _ = &mut pending => panic!("resolution should remain pending"), _ = tokio::task::yield_now() => {} }
        }
    }).await.unwrap();
    drop(pending);
    assert_ne!(fixture.audits(1).await[0].status, "completed");
    assert_eq!(fixture.remaining().await, 2);
    fixture.finish().await;
    peer.inspect(|calls| assert_eq!(calls.len(), 1));
}

fn cached_reply(family: &str, sse: bool) -> Reply {
    let raw = String::from_utf8(native::reply(
        if family == "anthropic" {
            "messages"
        } else {
            "chat"
        },
        false,
        sse,
        true,
    ))
    .unwrap();
    let raw = if family == "anthropic" {
        raw.replace(
            "\"input_tokens\":2",
            "\"input_tokens\":2,\"cache_read_input_tokens\":4,\"cache_creation_input_tokens\":6",
        )
    } else {
        raw.replace(
            "\"prompt_tokens\":2",
            "\"prompt_tokens\":12,\"prompt_tokens_details\":{\"cached_tokens\":4}",
        )
        .replace("\"total_tokens\":5", "\"total_tokens\":15")
    };
    Reply::raw(200, raw.into_bytes()).header(
        "content-type",
        if sse {
            "text/event-stream"
        } else {
            "application/json"
        },
    )
}
