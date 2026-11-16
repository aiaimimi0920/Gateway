//! Direct Messages follows the same single refresh owner and cancellation/refund boundaries.
use super::super::test_support::{Peer, Reply};
use super::{
    declared_messages::{assert_wire, fixture, reply},
    *,
};
#[tokio::test]
async fn lobsterai_declared_messages_401_only_next_request_uses_refreshed_token() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let peer = Peer::start(vec![
                    Reply::raw(401, KEY.as_bytes().to_vec()),
                    Reply::json(json!({"code":0,"data":{"accessToken":"new-access"}})),
                    reply(!stream, false),
                ])
                .await;
                let fixture = fixture(&peer, false).await;
                super::super::transport::TEST_ORIGIN
                    .scope(peer.origin.clone(), async {
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        assert_ne!(response.status(), StatusCode::OK);
                        assert!(!drain(response.into_body()).await.contains(KEY));
                        assert_ne!(fixture.audits(1).await[0].status, "completed");
                        assert_eq!(fixture.remaining().await, 2);
                        tokio::time::timeout(Duration::from_secs(5), async {
                            loop {
                                if fixture.state.route_config.snapshot().document().providers[0]
                                    .credentials[0]
                                    .api_key
                                    .as_deref()
                                    == Some("new-access")
                                {
                                    break;
                                }
                                tokio::task::yield_now().await;
                            }
                        })
                        .await
                        .unwrap();
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::OK, "{text}");
                        assert!(text.contains("合成答案"));
                    })
                    .await;
                assert_eq!(
                    fixture
                        .audits(2)
                        .await
                        .iter()
                        .filter(|row| row.status == "completed")
                        .count(),
                    1
                );
                assert_eq!(fixture.remaining().await, 1);
                fixture.finish().await;
                peer.inspect(|calls| {
                    assert_eq!(calls.len(), 3, "no hidden inference replay");
                    assert_eq!(calls[0].path, "/api/proxy/v1/messages");
                    assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
                    assert_eq!(calls[1].path, "/api/auth/refresh");
                    assert_eq!(calls[1].body["refreshToken"], "synthetic-refresh");
                    assert_eq!(calls[2].path, "/api/proxy/v1/messages");
                    assert_eq!(calls[2].headers["authorization"], "Bearer new-access");
                });
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_declared_messages_invalid_json_and_stream_never_complete() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                for kind in 0..3 {
                    let invalid = match kind {
                        0 => Reply::json(json!({
                            "type":"message", "role":"assistant", "content":[],
                            "stop_reason":"tool_use", "usage":{"input_tokens":2,"output_tokens":3}
                        })),
                        1 => Reply::raw(200, native::reply("messages", false, true, false))
                            .header("content-type", "text/event-stream"),
                        _ => {
                            let mut raw = native::reply("messages", false, true, true);
                            raw.extend_from_slice(b"event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"synthetic-secret\"}}\n\n");
                            Reply::raw(200, raw).header("content-type", "text/event-stream")
                        }
                    };
                    let peer = Peer::start(vec![invalid]).await;
                    let fixture = fixture(&peer, false).await;
                    super::super::transport::TEST_ORIGIN
                        .scope(peer.origin.clone(), async {
                            let response = fixture.open(endpoint, body(endpoint, stream)).await;
                            let status = response.status();
                            let mut response_body = response.into_body();
                            let (text, stream_failed) = tokio::time::timeout(Duration::from_secs(10), async {
                                let mut bytes = Vec::new();
                                let mut stream_failed = false;
                                while let Some(frame) = response_body.frame().await {
                                    match frame {
                                        Ok(frame) => if let Ok(data) = frame.into_data() {
                                            bytes.extend_from_slice(&data);
                                        },
                                        Err(error) => {
                                            assert!(stream, "non-stream response body failed: {error:?}");
                                            let diagnostic = format!("{error:?}");
                                            assert!(diagnostic.contains("invalid_lobsterai_stream"), "{diagnostic}");
                                            assert!(!diagnostic.contains("synthetic-secret"), "{diagnostic}");
                                            stream_failed = true;
                                            break;
                                        }
                                    }
                                }
                                (String::from_utf8(bytes).unwrap(), stream_failed)
                            }).await.unwrap();
                            drop(response_body);
                            assert!(!text.contains("synthetic-secret"), "{text}");
                            assert!(status != StatusCode::OK || stream_failed || text.contains("error"), "{status}: {text}");
                            for terminal in ["response.completed", "event: message_stop", "[DONE]"] {
                                assert!(!text.contains(terminal), "{text}");
                            }
                        })
                        .await;
                    assert_ne!(fixture.audits(1).await[0].status, "completed");
                    assert_eq!(fixture.remaining().await, 2);
                    fixture.finish().await;
                    assert_wire(&peer, 1);
                }
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_declared_messages_cancellation_preserves_balance_semantics() {
    for cash in [false, true] {
        let peer = Peer::start(vec![Reply::stalled()]).await;
        let fixture = fixture(&peer, cash).await;
        let mut raw = body("chat/completions", false);
        raw["max_tokens"] = json!(32);
        let future = super::super::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), fixture.open("chat/completions", raw));
        let mut pending = Box::pin(future);
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let mut seen = false;
                peer.inspect(|calls| seen = !calls.is_empty());
                if seen {
                    break;
                }
                tokio::select! {
                    _ = &mut pending => panic!("send should remain pending"),
                    _ = tokio::task::yield_now() => {}
                }
            }
        })
        .await
        .unwrap();
        drop(pending);
        assert_ne!(fixture.audits(1).await[0].status, "completed");
        if cash {
            let store = AccessBalanceStore::from_state(&fixture.state).unwrap();
            let ledger = store.cash_ledger(&fixture.id).await.unwrap();
            assert_eq!(ledger.len(), 1);
            assert_eq!(ledger[0].status, "unresolved");
            assert!(ledger[0].amount_micros.is_none() && ledger[0].usage.is_none());
            let balance = store.cash_balance(&fixture.id).await.unwrap().unwrap();
            assert_eq!(balance.spent_micros, 0);
            assert!(balance.reserved_micros > 0);
            assert_eq!(balance.pending_requests, 1);
        } else {
            assert_eq!(fixture.remaining().await, 2);
        }
        fixture.finish().await;
        assert_wire(&peer, 1);
    }
}
