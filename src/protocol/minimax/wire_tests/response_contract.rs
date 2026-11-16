//! Native Messages 保留 thinking；未知 200 错误不得成功结算或自动重放。
use super::*;
#[tokio::test]
async fn minimax_messages_preserve_thinking_json_and_stream() {
    Box::pin(async {
        for stream in [false,true] {
            let mut bytes = native::reply("messages",false,stream,true);
            let expected = if stream {
                let raw=String::from_utf8(bytes).unwrap();
                let frame="event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"synthetic thinking\",\"signature\":\"synthetic-signature\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n";
                bytes=raw.replace("event: message_delta",&format!("{frame}event: message_delta")).into_bytes();
                Value::Null
            } else {
                let mut value:Value=serde_json::from_slice(&bytes).unwrap();
                value["content"].as_array_mut().unwrap().insert(0,json!({"type":"thinking","thinking":"synthetic thinking","signature":"synthetic-signature"}));
                bytes=serde_json::to_vec(&value).unwrap(); value
            };
            let peer=peer::Peer::raw(bytes).await;
            let fixture=Box::pin(Fixture::new(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(),async {
                let response=fixture.open("messages",body("messages",stream)).await;
                assert_eq!(response.status(),StatusCode::OK);
                let text=drain(response.into_body()).await;
                assert!(text.contains("synthetic thinking") && text.contains("synthetic-signature"),"{text}");
                if !stream { assert_eq!(serde_json::from_str::<Value>(&text).unwrap(),expected); }
                assert_eq!(fixture.audits(1).await[0].total_tokens,Some(5));
                assert_eq!(fixture.remaining().await,1);
                assert_wire(&peer,1,"messages");
            }).await;
            fixture.finish().await; peer.stop().await;
        }
    }).await;
}
#[tokio::test]
async fn minimax_invalid_success_body_refunds_without_replay() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for reply in [
                json!({"type":"error","error":{"message":KEY}}),
                json!({"type":"message","content":[]}),
            ] {
                let peer = peer::Peer::raw(serde_json::to_vec(&reply).unwrap()).await;
                let fixture = Box::pin(Fixture::new(&peer)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let response = fixture.open(endpoint, body(endpoint, false)).await;
                    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
                    let text = drain(response.into_body()).await;
                    assert!(
                        text.contains("minimax_response_failed") && !text.contains(KEY),
                        "{text}"
                    );
                    assert_ne!(fixture.audits(1).await[0].status, "completed");
                    assert_eq!(fixture.remaining().await, 2);
                    assert_wire(&peer, 1, endpoint);
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
