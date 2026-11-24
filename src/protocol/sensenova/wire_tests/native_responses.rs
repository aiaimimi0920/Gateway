//! Public Responses preserves the documented native inference contract.
use super::*;

#[tokio::test]
async fn sensenova_native_responses_preserves_reasoning_and_image() {
    Box::pin(async {
        let answer = json!({
            "id":"resp_fixture","object":"response","status":"completed","model":UPSTREAM_MODEL,
            "output":[{"type":"reasoning","summary":[{"type":"summary_text","text":"summary"}]},
                      {"type":"message","role":"assistant","content":[{"type":"output_text","text":"answer"}]}],
            "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5,"output_tokens_details":{"reasoning_tokens":1}}
        });
        let peer = peer::Peer::raw(answer.to_string().into_bytes()).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let image = json!({"type":"input_image","image_url":"data:image/png;base64,aW1hZ2U="});
            let response = fixture.open("responses", json!({
                "model":MODEL,"store":false,"reasoning":{"summary":"auto"},
                "input":[{"role":"user","content":[{"type":"input_text","text":"describe"},image]}]
            })).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            {
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), 1, "{text}");
                let (path, headers, body) = &calls[0];
                assert_eq!(path, "/v1/responses", "{text}");
                assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
                assert_eq!(body["model"], UPSTREAM_MODEL);
                assert_eq!(body["reasoning"], json!({"summary":"auto"}));
                assert_eq!(body["input"][0]["content"][1], image);
            }
            assert_eq!(status, StatusCode::OK, "{text}");
            let value: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(value["output"], answer["output"]);
            assert_eq!(value["usage"], answer["usage"]);
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
async fn sensenova_native_responses_stream_admits_documented_compact_terminal() {
    Box::pin(async {
        let raw = [
            json!({"type":"response.created","sequence_number":0,"response":{"id":"resp_stream","object":"response","status":"in_progress","output":[],"model":UPSTREAM_MODEL}}),
            json!({"type":"response.reasoning_summary_text.delta","sequence_number":3,"delta":"native summary","usage":{"input_tokens":9999,"output_tokens":9999}}),
            json!({"type":"response.output_text.delta","sequence_number":4,"output_index":1,"item_id":"msg","content_index":0,"delta":"native answer"}),
            json!({"type":"response.incomplete","sequence_number":9,"response":{"id":"resp_stream","status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5,"output_tokens_details":{"reasoning_tokens":1}}}}),
        ].into_iter().map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap())).collect::<String>();
        let peer = peer::Peer::raw(raw.into_bytes()).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open("responses", body("responses", true)).await;
            assert_eq!(response.status(), StatusCode::OK);
            let text = drain(response.into_body()).await;
            assert!(text.contains("native summary") && text.contains("native answer"), "{text}");
            assert!(text.contains("response.incomplete") && text.contains("max_output_tokens"), "{text}");
            assert!(text.contains(MODEL) && !text.contains(UPSTREAM_MODEL) && !text.contains("[DONE]"), "{text}");
            let rows = fixture.audits(1).await;
            assert_identity(&rows[0]);
            assert_eq!(rows[0].status, "completed");
            assert_eq!((rows[0].prompt_tokens, rows[0].completion_tokens, rows[0].total_tokens), (Some(2), Some(3), Some(5)));
            assert_eq!(fixture.remaining().await, 1);
            assert_wire(&peer, 1, "responses");
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }).await;
}
