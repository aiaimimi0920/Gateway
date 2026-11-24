//! Public Messages must go to the documented Messages endpoint without a lossy Chat bridge.
use super::*;

#[tokio::test]
async fn sensenova_native_messages_preserves_thinking_and_image() {
    Box::pin(async {
        let answer = json!({
            "id":"msg_fixture","type":"message","role":"assistant","model":UPSTREAM_MODEL,
            "content":[{"type":"thinking","thinking":"native thought","signature":"opaque"},
                       {"type":"text","text":"native answer"}],
            "stop_reason":"end_turn","usage":{"input_tokens":2,"output_tokens":3,"reasoning_tokens":1}
        });
        let peer = peer::Peer::raw(answer.to_string().into_bytes()).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let image = json!({"type":"image","source":{"type":"base64","media_type":"image/png","data":"aW1hZ2U="}});
            let response = fixture.open("messages", json!({
                "model":MODEL,"max_tokens":32,"system":"policy",
                "thinking":{"type":"enabled"},"output_config":{"effort":"high"},
                "messages":[{"role":"user","content":[{"type":"text","text":"describe"},image]}]
            })).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            {
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), 1);
                let (path, headers, body) = &calls[0];
                assert_eq!(path, "/v1/messages", "{text}");
                assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
                assert!(!headers.contains_key("x-api-key"));
                assert_eq!(body["model"], UPSTREAM_MODEL);
                assert_eq!(body["thinking"]["type"], "enabled");
                assert_eq!(body["output_config"]["effort"], "high");
                assert_eq!(body["messages"][0]["content"][1], image);
            }
            assert_eq!(status, StatusCode::OK, "{text}");
            let value: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(value["content"], answer["content"]);
            assert_eq!(value["model"], MODEL);
            let rows = fixture.audits(1).await;
            assert_identity(&rows[0]);
            assert_eq!(rows[0].status, "completed");
            assert_eq!(rows[0].total_tokens, Some(5));
            assert_eq!(fixture.remaining().await, 1);
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sensenova_native_messages_stream_keeps_unsigned_thinking_and_usage() {
    Box::pin(async {
        let frame = |value: Value| {
            let kind = value["type"].as_str().unwrap();
            format!("event: {kind}\ndata: {value}\n\n")
        };
        let raw = [
            json!({"type":"message_start","message":{"id":"msg_stream","type":"message","role":"assistant","model":UPSTREAM_MODEL,"content":[],"usage":{"input_tokens":2,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"unsigned reasoning"},"usage":{"input_tokens":9999}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"native answer"}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3,"reasoning_tokens":1}}),
            json!({"type":"message_stop"}),
        ].into_iter().map(frame).collect::<String>();
        let peer = peer::Peer::raw(raw.into_bytes()).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open("messages", body("messages", true)).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            assert!(text.contains("unsigned reasoning") && text.contains("native answer"), "{text}");
            assert!(text.contains("message_stop") && text.contains(MODEL), "{text}");
            assert!(!text.contains(UPSTREAM_MODEL), "{text}");
            let rows = fixture.audits(1).await;
            assert_identity(&rows[0]);
            assert_eq!(rows[0].status, "completed");
            assert_eq!((rows[0].prompt_tokens, rows[0].completion_tokens, rows[0].total_tokens), (Some(2), Some(3), Some(5)));
            assert_eq!(fixture.remaining().await, 1);
            assert_wire(&peer, 1, "messages");
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sensenova_native_messages_rejects_hosted_tools_before_send() {
    Box::pin(async {
        let peer = peer::Peer::new("messages", false, false, 200).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut request = body("messages", false);
            request["tools"] = json!([{"type":"web_search_20250305","name":"web_search"}]);
            let response = fixture.open("messages", request).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
            assert!(text.contains("invalid_sensenova_token_plan"), "{text}");
            assert!(peer.calls.lock().unwrap().is_empty());
            assert_eq!(fixture.remaining().await, 2);
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
