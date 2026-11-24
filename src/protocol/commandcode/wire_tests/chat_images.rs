//! Public Chat images keep their URL and detail on the selected native wire.
use super::*;

#[tokio::test]
async fn commandcode_chat_image_urls_survive_json_and_sse_without_model_guessing() {
    for stream in [false, true] {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut request = body("chat/completions", stream);
            request["messages"][0]["content"] = json!([
                {"type":"text","text":"合成图片"},
                {"type":"image_url","image_url":{"url":"https://image.invalid/a.png","detail":"high"}},
                {"type":"image_url","image_url":{"url":"data:image/png;base64,aW1hZ2U="}}
            ]);
            let response = fixture.open("chat/completions", request.clone()).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            assert!(!text.contains(UPSTREAM_MODEL), "{text}");
            if stream {
                let frames = text.lines().filter_map(|line| line.strip_prefix("data: "));
                let frames: Vec<_> = frames.collect();
                assert_eq!(frames.last(), Some(&"[DONE]"), "{text}");
                assert!(frames.len() >= 3, "{text}");
                for frame in &frames[..frames.len() - 1] {
                    let frame: serde_json::Value = serde_json::from_str(frame).unwrap();
                    assert_eq!(frame["model"], MODEL);
                }
            } else {
                let reply: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(reply["model"], MODEL);
            }
            let rows = fixture.audits(1).await;
            assert_identity(&rows[0]);
            assert_eq!(rows[0].status, "completed");
            assert_eq!(rows[0].total_tokens, Some(5));
            assert_eq!(fixture.remaining().await, 1);
            assert_wire(&peer, 1, "chat");
            assert_eq!(peer.calls.lock().unwrap()[0].2["messages"], request["messages"]);
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn commandcode_malformed_and_non_image_parts_do_not_send_or_charge() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let fixture = Box::pin(Fixture::new(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        for part in [
            json!({"type":"image_url","image_url":{"url":"file:///image.png"}}),
            json!({"type":"image_url","image_url":{"url":"https://image.invalid/a","detail":42}}),
            json!({"type":"image_url","image_url":{"url":"https://image.invalid/a","extra":true}}),
            json!({"type":"input_audio","input_audio":{"data":"YQ==","format":"wav"}}),
        ] {
            let mut request = body("chat/completions", false);
            request["messages"][0]["content"] = json!([part]);
            let response = fixture.open("chat/completions", request).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
            assert_eq!(fixture.remaining().await, 2);
        }
        assert!(peer.calls.lock().unwrap().is_empty());
    })
    .await;
    fixture.finish().await;
    peer.stop().await;
}
