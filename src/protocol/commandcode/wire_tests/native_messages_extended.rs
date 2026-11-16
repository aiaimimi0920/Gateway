//! Rich native Messages traverse the public prepaid stream without canonical reconstruction.
use super::super::messages_stream_extended_tests::{encode, events};
use super::*;

#[tokio::test]
async fn commandcode_native_messages_rich_sse_keeps_content_usage_and_alias() {
    for reason in [
        "end_turn",
        "pause_turn",
        "refusal",
        "model_context_window_exceeded",
    ] {
        let mut events = events();
        events[19]["delta"]["stop_reason"] = reason.into();
        let peer = peer::Peer::raw(encode(&events)).await;
        let fixture = Box::pin(Fixture::native_messages(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open("messages", body("messages", true)).await;
            assert_eq!(response.status(), StatusCode::OK);
            let text = drain(response.into_body()).await;
            let values: Vec<Value> = text
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .map(|data| serde_json::from_str(data).unwrap())
                .collect();
            events[0]["message"]["model"] = MODEL.into();
            assert_eq!(values, events);
            let rows = fixture.audits(1).await;
            assert_identity(&rows[0]);
            assert_eq!(rows[0].status, "completed");
            assert_eq!(rows[0].total_tokens, Some(5));
            assert_eq!(fixture.remaining().await, 1);
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "/provider/v1/messages");
            assert_eq!(calls[0].1["authorization"], format!("Bearer {KEY}"));
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn commandcode_native_messages_rich_sse_tail_error_refunds_without_replay() {
    let mut events = events();
    events.push(json!({"type":"error","error":{"message":KEY}}));
    let peer = peer::Peer::raw(encode(&events)).await;
    let fixture = Box::pin(Fixture::native_messages(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        use futures::StreamExt;
        let response = fixture.open("messages", body("messages", true)).await;
        let mut source = response.into_body().into_data_stream();
        let mut published = Vec::new();
        while let Some(chunk) = source.next().await {
            match chunk {
                Ok(bytes) => published.extend_from_slice(&bytes),
                Err(_) => break,
            }
        }
        drop(source);
        let text = String::from_utf8(published).unwrap();
        assert!(!text.contains(KEY) && !text.contains("event: message_stop"));
        assert_ne!(fixture.audits(1).await[0].status, "completed");
        assert_eq!(fixture.remaining().await, 2);
        assert_eq!(peer.calls.lock().unwrap().len(), 1);
    })
    .await;
    fixture.finish().await;
    peer.stop().await;
}
