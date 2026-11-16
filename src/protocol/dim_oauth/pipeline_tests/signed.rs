//! Reuse the existing ordered Gemini-turn framework with DIM's own explicit source identity.
use super::*;
fn parts() -> Value {
    json!([
        {"text":"summary-only","thought":true,"thoughtSignature":"c3VtbWFyeQ=="},
        {"text":"合成答案"},
        {"functionCall":{"name":"lookup","args":native::args()},"thoughtSignature":"YWJj"},
        {"text":"","thoughtSignature":"ZGVm"}
    ])
}
fn reply() -> Vec<u8> {
    format!("data: {}\n\n",json!({"candidates":[{"content":{"parts":parts()},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}})).into_bytes()
}
fn turn(text: &str, endpoint: &str, stream: bool) -> Value {
    if !stream {
        return serde_json::from_str::<Value>(text).unwrap()["gateway_vertex_turn"].clone();
    }
    let frames = public_wire::frames(text);
    let turns: Vec<_> = frames
        .iter()
        .filter_map(|v| {
            if endpoint == "responses" {
                v["response"].get("gateway_vertex_turn")
            } else {
                v.get("gateway_vertex_turn")
            }
        })
        .collect();
    assert_eq!(turns.len(), 1, "{text}");
    turns[0].clone()
}
fn followup(endpoint: &str, stream: bool, calls: &[public_wire::Call], turn: Value) -> Value {
    let mut req = public_wire::followup(endpoint, stream, calls);
    match endpoint {
        "messages" => req["messages"][1]["content"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"type":"text","text":"合成答案"})),
        "responses" => req["input"].as_array_mut().unwrap().insert(
            1,
            json!({"role":"assistant","content":[{"type":"output_text","text":"合成答案"}]}),
        ),
        _ => req["messages"][1]["content"] = json!("合成答案"),
    }
    req["gateway_vertex_history"] = json!([{"assistant_turn":0,"turn":turn}]);
    req
}
#[tokio::test]
async fn dim_public_gemini_ordered_state_roundtrips_and_rejects_both_vertex_sources() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let peer = peer::Peer::raw(reply()).await;
                let fixture = Box::pin(Fixture::new(&peer, "gemini")).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let response = fixture
                        .open(endpoint, public_wire::request_body(endpoint, stream))
                        .await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    let calls = public_wire::calls(&text, endpoint, stream);
                    assert_eq!(calls.len(), 1);
                    let turn = turn(&text, endpoint, stream);
                    assert_eq!(turn["source"], "dim_user_oauth");
                    assert_eq!(turn["model"], "opaque-model");
                    assert_eq!(turn["parts"][0]["thought_signature"], "c3VtbWFyeQ==");
                    assert_eq!(turn["parts"][3]["text"], "");
                    assert_eq!(fixture.audits(1).await[0].status, "completed");
                    for (source, count) in [("vertex_user_oauth", 2), ("vertex_express_api", 3)] {
                        let mut foreign = turn.clone();
                        foreign["source"] = json!(source);
                        let response = fixture
                            .open(endpoint, followup(endpoint, stream, &calls, foreign))
                            .await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
                        assert!(text.contains("dim_oauth_invalid_signed_history"), "{text}");
                        assert_eq!(
                            fixture
                                .audits(count)
                                .await
                                .iter()
                                .filter(|r| r.status == "failed")
                                .count(),
                            count - 1
                        );
                        assert_eq!(fixture.remaining().await, 1);
                        assert_eq!(peer.calls.lock().unwrap().len(), 1);
                    }
                    let response = fixture
                        .open(endpoint, followup(endpoint, stream, &calls, turn))
                        .await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    assert_eq!(
                        fixture
                            .audits(4)
                            .await
                            .iter()
                            .filter(|r| r.status == "completed")
                            .count(),
                        2
                    );
                    assert_eq!(fixture.remaining().await, 0);
                    let captured = peer.calls.lock().unwrap();
                    assert_eq!(captured.len(), 2);
                    assert_eq!(captured[1].2["contents"][1]["parts"], parts());
                    assert!(captured[1].2.get("gateway_vertex_history").is_none());
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
#[tokio::test]
async fn dim_public_buffered_xml_text_never_becomes_a_native_tool_call() {
    Box::pin(async {
        let xml="<tool_calls><tool_call><tool_name>lookup</tool_name><parameters>{\"city\":\"杭州\"}</parameters></tool_call></tool_calls>";
        for endpoint in ENDPOINTS {
            let raw=format!("data: {}\n\n",json!({"candidates":[{"content":{"parts":[{"text":xml}]},"finishReason":"STOP"}]})).into_bytes();
            let peer=peer::Peer::raw(raw).await;let fixture=Box::pin(Fixture::new(&peer,"gemini")).await;
            super::super::test_support::origin(peer.origin.clone(),async {
                let response=fixture.open(endpoint,public_wire::request_body(endpoint,false)).await;
                let status=response.status();let text=drain(response.into_body()).await;assert_eq!(status,StatusCode::OK,"{text}");
                let value:Value=serde_json::from_str(&text).unwrap();
                match endpoint{
                    "responses"=>assert!(value["output"].as_array().unwrap().iter().all(|v|v["type"]!="function_call")),
                    "messages"=>assert!(value["content"].as_array().unwrap().iter().all(|v|v["type"]!="tool_use")),
                    _=>assert!(value["choices"][0]["message"].get("tool_calls").is_none_or(|v|v.as_array().unwrap().is_empty()))
                }
                assert!(text.contains("<tool_calls>"),"{text}");assert_eq!(fixture.audits(1).await[0].status,"completed");
            }).await;fixture.finish().await;peer.stop().await;
        }
    }).await;
}
