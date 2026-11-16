//! Real public routing must select the declared native wire, not merely a different packer unit test.
use super::*;

fn wire(peer: &peer::Peer, count: usize) {
    let calls = peer.calls.lock().unwrap();
    assert_eq!(calls.len(), count);
    for (path, headers, body) in calls.iter() {
        assert_eq!(path, "/provider/v1/responses");
        assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
        assert!(!headers.contains_key("cookie") && !headers.contains_key("originator"));
        assert_eq!(body["model"], UPSTREAM_MODEL);
        assert!(body.get("input").is_some() && body.get("messages").is_none());
        assert!(!body.to_string().contains("gateway_commandcode_catalog"));
    }
}

#[tokio::test]
async fn commandcode_native_responses_public_json_sse_tools_and_billing() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = peer::Peer::new("responses", true, false, 200).await;
            let fixture = Box::pin(Fixture::native_responses(&peer)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let mut first = public_wire::request_body("responses", stream);
                first["model"] = MODEL.into();
                first["tool_choice"] = "auto".into();
                first["instructions"] = "first instruction".into();
                first["input"] = json!([
                    {"role":"system","content":[{"type":"input_text","text":"second instruction"},{"type":"input_text","text":"third instruction"}]},
                    {"role":"user","content":"question"}
                ]);
                let response = fixture.open("responses", first).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                let calls = public_wire::calls(&text, "responses", stream);
                assert_eq!(calls.len(), 2, "{text}");
                let mut next = public_wire::followup("responses", stream, &calls);
                next["model"] = MODEL.into();
                let response = fixture.open("responses", next).await;
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
                assert_eq!(sent["instructions"], "first instruction");
                assert_eq!(sent["input"][0]["content"][0]["text"], "second instruction");
                assert_eq!(sent["input"][0]["content"][1]["text"], "third instruction");
                native::assert_tools(&peer.calls.lock().unwrap(), "responses", &calls);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn commandcode_native_responses_failed_body_refunds_without_replay() {
    let peer = peer::Peer::raw(
        json!({"id":"r","object":"response","status":"failed","output":[],
        "error":{"message":KEY}})
        .to_string()
        .into_bytes(),
    )
    .await;
    let fixture = Box::pin(Fixture::native_responses(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        let response = fixture.open("responses", body("responses", false)).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(!drain(response.into_body()).await.contains(KEY));
        assert_eq!(fixture.remaining().await, 2);
        assert_ne!(fixture.audits(1).await[0].status, "completed");
        wire(&peer, 1);
    })
    .await;
    fixture.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_native_responses_failed_sse_is_redacted_and_not_billed() {
    for raw in [
        format!("data: {{\"type\":\"response.cancelled\",\"response\":{{\"status\":\"cancelled\"}},\"message\":\"{KEY}\"}}\n\n").into_bytes(),
        format!("data: {{\"type\":\"response.failed\",\"response\":{{\"status\":\"failed\",\"error\":{{\"message\":\"{KEY}\"}}}}}}\n\n").into_bytes(),
        native::reply("responses", false, true, false),
    ] {
        let peer = peer::Peer::raw(raw).await;
        let fixture = Box::pin(Fixture::native_responses(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open("responses", body("responses", true)).await;
            use futures::StreamExt;
            let mut source = response.into_body().into_data_stream();
            let mut published = Vec::new();
            while let Some(chunk) = source.next().await {
                match chunk { Ok(bytes) => published.extend_from_slice(&bytes), Err(_) => break }
            }
            let text = String::from_utf8_lossy(&published);
            drop(source);
            assert!(!text.contains(KEY) && !text.contains("response.completed"), "{text}");
            assert_ne!(fixture.audits(1).await[0].status, "completed");
            assert_eq!(fixture.remaining().await, 2);
            wire(&peer, 1);
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }
}
