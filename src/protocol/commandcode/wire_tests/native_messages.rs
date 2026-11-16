//! Declared Messages must reach the native wire through the public prepaid pipeline.
use super::*;

fn wire(peer: &peer::Peer, count: usize) {
    let calls = peer.calls.lock().unwrap();
    assert_eq!(calls.len(), count);
    for (path, headers, body) in calls.iter() {
        assert_eq!(path, "/provider/v1/messages");
        assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
        for name in ["x-api-key", "cookie", "originator", "chatgpt-account-id"] {
            assert!(!headers.contains_key(name));
        }
        assert_eq!(body["model"], UPSTREAM_MODEL);
        assert!(body.get("messages").is_some() && body.get("input").is_none());
        assert!(body.get("cache_control").is_none());
        assert!(!body.to_string().contains("gateway_commandcode_catalog"));
    }
}

#[tokio::test]
async fn commandcode_native_messages_public_json_sse_tools_and_billing() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = peer::Peer::new("messages", true, false, 200).await;
            let fixture = Box::pin(Fixture::native_messages(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let mut first = public_wire::request_body("messages", stream);
                first["model"] = MODEL.into();
                first["system"] =
                    json!([{"type":"text","text":"first"},{"type":"text","text":"second"}]);
                first["tool_choice"] = json!({"type":"auto"});
                let response = fixture.open("messages", first).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains(MODEL), "{text}");
                let calls = public_wire::calls(&text, "messages", stream);
                assert_eq!(calls.len(), 2, "{text}");
                let mut next = public_wire::followup("messages", stream, &calls);
                next["model"] = MODEL.into();
                let response = fixture.open("messages", next).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains("合成答案"), "{text}");
                for row in fixture.audits(2).await {
                    assert_identity(&row);
                    assert_eq!(row.status, "completed");
                    assert_eq!(row.total_tokens, Some(5));
                }
                assert_eq!(fixture.remaining().await, 0);
                wire(&peer, 2);
                let sent = peer.calls.lock().unwrap()[0].2.clone();
                assert_eq!(sent["system"][0]["text"], "first");
                assert_eq!(sent["system"][1]["text"], "second");
                assert_eq!(sent["tool_choice"], json!({"type":"auto"}));
                native::assert_tools(&peer.calls.lock().unwrap(), "messages", &calls);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn commandcode_native_messages_failures_refund_without_replay_or_secret_leaks() {
    let mut tail_error = native::reply("messages", false, true, true);
    tail_error.extend_from_slice(
        format!(
            "event: error\ndata: {{\"type\":\"error\",\"error\":{{\"message\":\"{KEY}\"}}}}\n\n"
        )
        .as_bytes(),
    );
    for (stream, raw) in [
        (true, super::super::messages_stream_tests::tool_reply("{")),
        (false, json!({"type":"error","error":{"message":KEY}}).to_string().into_bytes()),
        (true, format!("event: error\ndata: {{\"type\":\"error\",\"error\":{{\"message\":\"{KEY}\"}}}}\n\n").into_bytes()),
        (true, native::reply("messages", false, true, false)),
        (true, tail_error),
    ] {
        let peer = peer::Peer::raw(raw).await;
        let fixture = Box::pin(Fixture::native_messages(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open("messages", body("messages", stream)).await;
            if !stream { assert_eq!(response.status(), StatusCode::BAD_GATEWAY); }
            use futures::StreamExt;
            let mut source = response.into_body().into_data_stream();
            let mut published = Vec::new();
            while let Some(chunk) = source.next().await {
                match chunk { Ok(bytes) => published.extend_from_slice(&bytes), Err(_) => break }
            }
            drop(source);
            let text = String::from_utf8_lossy(&published);
            assert!(!text.contains(KEY) && !text.contains("event: message_stop"), "{text}");
            assert_ne!(fixture.audits(1).await[0].status, "completed");
            assert_eq!(fixture.remaining().await, 2);
            wire(&peer, 1);
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn commandcode_native_messages_data_only_frames_keep_usage_and_alias() {
    let raw = String::from_utf8(native::reply("messages", false, true, true)).unwrap();
    let raw = raw
        .lines()
        .filter(|line| !line.starts_with("event:"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let peer = peer::Peer::raw(raw.into_bytes()).await;
    let fixture = Box::pin(Fixture::native_messages(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        let response = fixture.open("messages", body("messages", true)).await;
        let text = drain(response.into_body()).await;
        assert!(
            text.contains("event: message_stop") && text.contains(MODEL),
            "{text}"
        );
        let rows = fixture.audits(1).await;
        assert_eq!(rows[0].status, "completed");
        assert_eq!(rows[0].total_tokens, Some(5));
        assert_eq!(fixture.remaining().await, 1);
        wire(&peer, 1);
    })
    .await;
    fixture.finish().await;
    peer.stop().await;
}
