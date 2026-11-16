//! Native replies retain opaque protocol content while the public pipeline owns billing.
use super::*;

fn reply(endpoint: &str) -> Value {
    if endpoint == "responses" {
        json!({"id":"resp-native","object":"response","model":UPSTREAM_MODEL,
            "status":"completed","metadata":{"trace":"synthetic"},
            "output":[
                {"id":"rs-1","type":"reasoning","summary":[{"type":"summary_text","text":"reason"}],"encrypted_content":"opaque"},
                {"id":"msg-1","type":"message","role":"assistant","status":"completed",
                 "content":[{"type":"refusal","refusal":"cannot comply"},
                            {"type":"output_text","text":"answer","annotations":[{"type":"url_citation","url":"https://example.com","title":"source","start_index":0,"end_index":6}]}]},
                {"id":"custom-1","type":"custom_tool_call","call_id":"call-1","name":"grammar","input":"opaque command"},
                {"id":"web-1","type":"web_search_call","status":"completed","action":{"type":"search","query":"synthetic"}}
            ],"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5,"output_tokens_details":{"reasoning_tokens":1}}})
    } else {
        json!({"id":"msg-native","type":"message","role":"assistant","model":UPSTREAM_MODEL,
            "stop_reason":"end_turn","stop_sequence":null,
            "content":[{"type":"thinking","thinking":"reason","signature":"signed"},
                       {"type":"redacted_thinking","data":"opaque"},
                       {"type":"text","text":"answer","citations":[{"type":"char_location","document_index":0,"start_char_index":0,"end_char_index":6,"cited_text":"source"}]}],
            "usage":{"input_tokens":2,"output_tokens":3,"cache_read_input_tokens":0}})
    }
}

#[tokio::test]
async fn commandcode_native_json_preserves_rich_response_and_alias_with_billing() {
    for endpoint in ["responses", "messages"] {
        let mut expected = reply(endpoint);
        // Unrelated extension names are opaque content, not an instruction to invoke turn decoders.
        expected["gateway_sources"] = Value::Null;
        expected["gateway_bedrock_turn"] = json!({"opaque":"preserved"});
        let peer = peer::Peer::raw(expected.to_string().into_bytes()).await;
        let fixture = if endpoint == "responses" {
            Box::pin(Fixture::native_responses(&peer)).await
        } else {
            Box::pin(Fixture::native_messages(&peer)).await
        };
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = fixture.open(endpoint, body(endpoint, false)).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            let actual: Value = serde_json::from_str(&text).unwrap();
            let mut expected = expected.clone();
            expected["model"] = MODEL.into();
            assert_eq!(actual, expected);
            let rows = fixture.audits(1).await;
            assert_identity(&rows[0]);
            assert_eq!(rows[0].status, "completed");
            assert_eq!(rows[0].total_tokens, Some(5));
            assert_eq!(
                rows[0].route_trace.as_ref().unwrap()["canonicalCompletionSemantics"],
                if endpoint == "responses" {
                    "tool_calls"
                } else {
                    "stop"
                }
            );
            assert_eq!(fixture.remaining().await, 1);
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, format!("/provider/v1/{endpoint}"));
            assert_eq!(calls[0].1["authorization"], format!("Bearer {KEY}"));
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    }
}

#[test]
fn commandcode_native_json_does_not_widen_canonical_probe_admission() {
    assert!(super::super::responses::unpack(&reply("responses")).is_err());
    assert!(super::super::messages::unpack(&reply("messages")).is_err());
}

#[tokio::test]
async fn commandcode_native_json_invalid_envelope_or_usage_refunds_without_replay() {
    for endpoint in ["responses", "messages"] {
        let contents = if endpoint == "responses" {
            "output"
        } else {
            "content"
        };
        let terminal = if endpoint == "responses" {
            "status"
        } else {
            "stop_reason"
        };
        let base = reply(endpoint);
        let mut invalid = Vec::new();
        for (field, value) in [
            (terminal, json!("failed")),
            (contents, json!([{"type":""}])),
            (contents, json!([null])),
            ("usage", json!({"input_tokens":-1,"output_tokens":3})),
            ("usage", json!({"input_tokens":u64::MAX,"output_tokens":1})),
            ("error", json!({"message":KEY})),
        ] {
            let mut changed = base.clone();
            changed[field] = value;
            invalid.push(changed.to_string());
        }
        if endpoint == "responses" {
            let mut overflow = base.clone();
            overflow["gateway_sources"] = Value::Null;
            overflow["usage"] = json!({"prompt_tokens":u64::MAX,"completion_tokens":1});
            invalid.push(overflow.to_string());
        }
        let mut duplicate = base.to_string();
        duplicate.pop();
        duplicate.push_str(",\"usage\":null}");
        invalid.push(duplicate);
        for raw in invalid {
            let peer = peer::Peer::raw(raw.into_bytes()).await;
            let fixture = if endpoint == "responses" {
                Box::pin(Fixture::native_responses(&peer)).await
            } else {
                Box::pin(Fixture::native_messages(&peer)).await
            };
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture.open(endpoint, body(endpoint, false)).await;
                assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
                assert!(!drain(response.into_body()).await.contains(KEY));
                assert_eq!(fixture.remaining().await, 2);
                assert_ne!(fixture.audits(1).await[0].status, "completed");
                assert_eq!(peer.calls.lock().unwrap().len(), 1);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    }
}
