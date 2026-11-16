//! Final usage must stay unknown through public projection and the cash settlement owner.
use super::super::test_support::{Peer, Reply};
use super::*;
#[tokio::test]
async fn lobsterai_virtual_missing_final_output_keeps_cash_unresolved() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let raw = String::from_utf8(native::reply("messages", false, true, true)).unwrap();
                // Retain message_start's provisional zero, remove only the final output counter.
                let raw = raw.replace("\"output_tokens\":3", "\"unreported_output\":3");
                let peer = Peer::start(vec![
                    virtual_models::resolution("anthropic"),
                    Reply::raw(200, raw.into_bytes()).header("content-type", "text/event-stream"),
                ])
                .await;
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
                        raw[if endpoint == "responses" {
                            "max_output_tokens"
                        } else {
                            "max_tokens"
                        }] = json!(32);
                        let response = fixture.open(endpoint, raw).await;
                        assert_eq!(response.status(), StatusCode::OK);
                        assert!(drain(response.into_body()).await.contains("合成答案"));
                    })
                    .await;
                let audits = fixture.audits(1).await;
                assert_eq!(audits[0].status, "completed");
                assert_eq!(audits[0].total_tokens, None);
                let store = AccessBalanceStore::from_state(&fixture.state).unwrap();
                let ledger = store.cash_ledger(&fixture.id).await.unwrap();
                assert_eq!(ledger[0].status, "unresolved");
                assert_eq!(ledger[0].amount_micros, None);
                assert!(ledger[0].usage.is_none());
                let balance = store.cash_balance(&fixture.id).await.unwrap().unwrap();
                assert_eq!(balance.spent_micros, 0);
                assert!(balance.reserved_micros > 0);
                assert_eq!(balance.pending_requests, 1);
                fixture.finish().await;
                peer.inspect(|calls| assert_eq!(calls.len(), 2));
            }
        }
    })
    .await;
}
