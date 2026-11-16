//! Rich Responses must traverse the public stream without a lossy Chat round trip.
use super::*;

fn events(status: &str) -> Vec<Value> {
    let response = json!({"id":"resp-rich","object":"response","model":UPSTREAM_MODEL,
        "status":status,"incomplete_details":if status == "incomplete" { json!({"reason":"content_filter"}) } else { Value::Null },
        "output":[
            {"id":"rs-1","type":"reasoning","summary":[{"type":"summary_text","text":"reason"}],"encrypted_content":"opaque"},
            {"id":"msg-1","type":"message","role":"assistant","status":"completed","content":[{"type":"refusal","refusal":"cannot comply"}]},
            {"id":"custom-1","type":"custom_tool_call","call_id":"call-1","name":"grammar","input":"opaque command"},
            {"id":"web-1","type":"web_search_call","status":"completed","action":{"type":"search","query":"synthetic"}}
        ],"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5,"prompt_tokens":999,"completion_tokens":999}});
    vec![
        json!({"type":"response.created","sequence_number":0,"response":{"id":"resp-rich","object":"response","status":"in_progress","model":UPSTREAM_MODEL,"output":[]}}),
        json!({"type":"response.reasoning_summary_text.delta","sequence_number":1,"item_id":"rs-1","output_index":0,"summary_index":0,"delta":"reason"}),
        json!({"type":"response.refusal.delta","sequence_number":2,"item_id":"msg-1","output_index":1,"content_index":0,"delta":"cannot comply"}),
        json!({"type":"response.custom_tool_call_input.delta","sequence_number":3,"item_id":"custom-1","output_index":2,"delta":"opaque command"}),
        json!({"type":"response.web_search_call.completed","sequence_number":4,"item_id":"web-1","output_index":3}),
        json!({"type":"response.provider_trace","sequence_number":5,"response":{"usage":{"input_tokens":999,"output_tokens":999}},"status":"incomplete","output":[{"type":"function_call"}]}),
        json!({"type":format!("response.{status}"),"sequence_number":6,"response":response}),
    ]
}

fn encode(events: &[Value]) -> Vec<u8> {
    events
        .iter()
        .map(|value| {
            format!(
                "event: {}\ndata: {value}\n\n",
                value["type"].as_str().unwrap()
            )
        })
        .collect::<String>()
        .into_bytes()
}

#[tokio::test]
async fn commandcode_responses_native_observer_does_not_capture_chat_bridge() {
    // Keep the existing bridge semantics; native incomplete-details observation is lane-specific.
    let raw = String::from_utf8(native::reply("chat", false, true, true))
        .unwrap()
        .replace("\"finish_reason\":\"stop\"", "\"finish_reason\":\"length\"");
    let peer = peer::Peer::raw(raw.into_bytes()).await;
    let fixture = Box::pin(Fixture::new(&peer)).await;
    super::super::test_support::origin(peer.origin.clone(), async {
        let response = fixture.open("responses", body("responses", true)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(drain(response.into_body())
            .await
            .contains("response.incomplete"));
        let rows = fixture.audits(1).await;
        assert_eq!(rows[0].status, "completed");
        assert_eq!(rows[0].total_tokens, Some(5));
        assert!(rows[0].route_trace.as_ref().unwrap()["canonicalCompletionSemantics"].is_null());
        assert_eq!(fixture.remaining().await, 1);
        assert_eq!(
            peer.calls.lock().unwrap()[0].0,
            "/provider/v1/chat/completions"
        );
    })
    .await;
    fixture.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_native_responses_rich_sse_preserves_events_and_terminal_billing() {
    for (status, semantics) in [
        ("completed", "tool_calls"),
        ("incomplete", "content_filter"),
    ] {
        let mut expected = events(status);
        let peer = peer::Peer::raw(encode(&expected)).await;
        let fixture = Box::pin(Fixture::native_responses(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open("responses", body("responses", true)).await;
            assert_eq!(response.status(), StatusCode::OK);
            let text = drain(response.into_body()).await;
            let actual: Vec<Value> = text
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .map(|data| serde_json::from_str(data).unwrap())
                .collect();
            expected[0]["response"]["model"] = MODEL.into();
            expected[6]["response"]["model"] = MODEL.into();
            assert_eq!(actual, expected);
            let rows = fixture.audits(1).await;
            assert_identity(&rows[0]);
            assert_eq!(rows[0].status, "completed");
            assert_eq!(rows[0].total_tokens, Some(5));
            assert_eq!(
                rows[0].route_trace.as_ref().unwrap()["canonicalCompletionSemantics"],
                semantics
            );
            assert_eq!(fixture.remaining().await, 1);
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "/provider/v1/responses");
            assert_eq!(calls[0].1["authorization"], format!("Bearer {KEY}"));
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn commandcode_native_responses_rich_sse_invalid_terminal_refunds() {
    let base = events("completed");
    let mut invalid = Vec::new();
    for (field, value) in [
        ("status", json!("failed")),
        ("usage", json!({"input_tokens":u64::MAX,"output_tokens":1})),
        ("output", json!([null])),
    ] {
        let mut changed = base.clone();
        changed[6]["response"][field] = value;
        invalid.push(encode(&changed));
    }
    let mut tail = encode(&base);
    tail.extend_from_slice(
        format!("event: error\ndata: {{\"type\":\"error\",\"message\":\"{KEY}\"}}\n\n").as_bytes(),
    );
    invalid.push(tail);
    for raw in invalid {
        let peer = peer::Peer::raw(raw).await;
        let fixture = Box::pin(Fixture::native_responses(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            use futures::StreamExt;
            let response = fixture.open("responses", body("responses", true)).await;
            let mut stream = response.into_body().into_data_stream();
            let mut bytes = Vec::new();
            while let Some(Ok(chunk)) = stream.next().await {
                bytes.extend_from_slice(&chunk);
            }
            drop(stream);
            let text = String::from_utf8(bytes).unwrap();
            assert!(!text.contains(KEY) && !text.contains("event: response.completed"));
            assert_ne!(fixture.audits(1).await[0].status, "completed");
            assert_eq!(fixture.remaining().await, 2);
            assert_eq!(peer.calls.lock().unwrap().len(), 1);
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    }
}
