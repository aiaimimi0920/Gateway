//! Catalog-selected Messages must retain cache costs and unknown final output through settlement.
use super::super::test_support::{Peer, Reply};
use super::{
    declared_messages::{assert_wire, fixture, DECLARED},
    *,
};
fn cached(sse: bool, complete_usage: bool) -> Reply {
    let raw = String::from_utf8(native::reply("messages", false, sse, true)).unwrap();
    let raw = raw.replace(
        "\"input_tokens\":2",
        "\"input_tokens\":2,\"cache_read_input_tokens\":4,\"cache_creation_input_tokens\":6",
    );
    let raw = if complete_usage {
        raw
    } else {
        raw.replace("\"output_tokens\":3", "\"unreported_output\":3")
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
#[tokio::test]
async fn lobsterai_declared_messages_cash_cache_and_unknown_usage() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                for sse in [false, true] {
                    for known in [false, true] {
                        let peer = Peer::start(vec![cached(sse, known)]).await;
                        let fixture = fixture(&peer, true).await;
                        super::super::transport::TEST_ORIGIN
                            .scope(peer.origin.clone(), async {
                                let mut raw = body(endpoint, stream);
                                raw.as_object_mut().unwrap().remove("max_tokens");
                                raw[if endpoint == "responses" {
                                    "max_output_tokens"
                                } else {
                                    "max_completion_tokens"
                                }] = json!(32);
                                let response = fixture.open(endpoint, raw).await;
                                let status = response.status();
                                let text = drain(response.into_body()).await;
                                assert_eq!(
                                    status,
                                    StatusCode::OK,
                                    "{endpoint}/{stream}/{sse}/{known}: {text}"
                                );
                                assert!(text.contains("合成答案"));
                                if known {
                                    if endpoint == "messages" {
                                        assert!(text.contains("\"input_tokens\":2"), "{text}");
                                        assert!(
                                            text.contains("\"cache_read_input_tokens\":4"),
                                            "{text}"
                                        );
                                        assert!(
                                            text.contains("\"cache_creation_input_tokens\":6"),
                                            "{text}"
                                        );
                                    } else {
                                        assert!(
                                            text.contains(if endpoint == "responses" {
                                                "\"input_tokens\":12"
                                            } else {
                                                "\"prompt_tokens\":12"
                                            }),
                                            "{text}"
                                        );
                                    }
                                }
                            })
                            .await;
                        let audits = fixture.audits(1).await;
                        assert_eq!(audits[0].status, "completed");
                        assert_eq!(audits[0].total_tokens, if known { Some(5) } else { None });
                        let store = AccessBalanceStore::from_state(&fixture.state).unwrap();
                        let ledger = store.cash_ledger(&fixture.id).await.unwrap();
                        assert_eq!(ledger.len(), 1);
                        let row = &ledger[0];
                        let balance = store.cash_balance(&fixture.id).await.unwrap().unwrap();
                        if known {
                            assert_eq!(row.status, "settled");
                            assert_eq!(row.amount_micros, Some(18));
                            let quote = row.settled_quote.as_ref().unwrap();
                            assert_eq!(quote.model, DECLARED);
                            assert!(!quote.cache_tokens_separate);
                            let usage = row.usage.as_ref().unwrap();
                            assert_eq!(
                                (
                                    usage.prompt_tokens,
                                    usage.completion_tokens,
                                    usage.total_tokens
                                ),
                                (12, 3, 15)
                            );
                            assert_eq!(usage.cache_read_input_tokens, Some(4));
                            assert_eq!(usage.cache_creation_input_tokens, Some(6));
                            assert_eq!(
                                (
                                    balance.spent_micros,
                                    balance.reserved_micros,
                                    balance.pending_requests
                                ),
                                (18, 0, 0)
                            );
                        } else {
                            assert_eq!(row.status, "unresolved");
                            assert!(row.amount_micros.is_none() && row.usage.is_none());
                            assert_eq!(balance.spent_micros, 0);
                            assert!(balance.reserved_micros > 0);
                            assert_eq!(balance.pending_requests, 1);
                        }
                        fixture.finish().await;
                        assert_wire(&peer, 1);
                        peer.inspect(|calls| {
                            assert_eq!(calls[0].body["max_tokens"], 32);
                            assert!(calls[0].body.get("max_output_tokens").is_none());
                            assert!(calls[0].body.get("max_completion_tokens").is_none());
                        });
                    }
                }
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_declared_messages_failed_inference_keeps_cash_unresolved() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = Peer::start(vec![Reply::raw(403, KEY.as_bytes().to_vec())]).await;
            let fixture = fixture(&peer, true).await;
            let mut raw = body("chat/completions", stream);
            raw["max_tokens"] = json!(32);
            let response = super::super::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), fixture.open("chat/completions", raw))
                .await;
            assert_ne!(response.status(), StatusCode::OK);
            assert!(!drain(response.into_body()).await.contains(KEY));
            assert_ne!(fixture.audits(1).await[0].status, "completed");
            let store = AccessBalanceStore::from_state(&fixture.state).unwrap();
            let ledger = store.cash_ledger(&fixture.id).await.unwrap();
            assert_eq!(ledger.len(), 1);
            assert_eq!(ledger[0].status, "unresolved");
            assert!(ledger[0].amount_micros.is_none() && ledger[0].usage.is_none());
            let balance = store.cash_balance(&fixture.id).await.unwrap().unwrap();
            assert_eq!(balance.spent_micros, 0);
            assert!(balance.reserved_micros > 0);
            assert_eq!(balance.pending_requests, 1);
            fixture.finish().await;
            assert_wire(&peer, 1);
        }
    })
    .await;
}
