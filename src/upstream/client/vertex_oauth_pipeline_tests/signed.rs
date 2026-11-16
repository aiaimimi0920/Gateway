//! Independent Full Vertex roundtrip: a product-scoped extension never borrows Express identity.
use super::{
    tools::{self, public_wire},
    *,
};
mod failures;

fn parts() -> Value {
    json!([
        {"text":"summary-only","thought":true,"thoughtSignature":"c3VtbWFyeQ=="},
        {"text":"A"},
        {"functionCall":{"name":"lookup","args":{"city":"杭州","nested":[1,2],"literal":"&amp; &quot;"}},"thoughtSignature":"YWJj"},
        {"text":"B"},
        {"functionCall":{"name":"lookup","args":{"city":"杭州","nested":[1,2]}}},
        {"text":"","thoughtSignature":"ZGVm"}
    ])
}
fn signed_body() -> Vec<u8> {
    event(
        json!({"candidates":[{"content":{"parts":parts()},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}}),
    )
}
fn turn(text: &str, endpoint: &str, stream: bool) -> Value {
    if !stream {
        return serde_json::from_str::<Value>(text).unwrap()["gateway_vertex_turn"].clone();
    }
    let frames = public_wire::frames(text);
    let turns: Vec<_> = frames
        .iter()
        .filter_map(|f| {
            let body = if endpoint == "responses" {
                &f["response"]
            } else {
                f
            };
            body.get("gateway_vertex_turn")
        })
        .collect();
    assert_eq!(turns.len(), 1, "terminal extension exactly once: {text}");
    turns[0].clone()
}
fn followup(endpoint: &str, stream: bool, calls: &[public_wire::Call], turn: Value) -> Value {
    let mut req = public_wire::followup(endpoint, stream, calls);
    match endpoint {
        "messages" => req["messages"][1]["content"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"type":"text","text":"AB"})),
        "responses" => req["input"].as_array_mut().unwrap().insert(
            1,
            json!({"role":"assistant","content":[{"type":"output_text","text":"AB"}]}),
        ),
        _ => req["messages"][1]["content"] = json!("AB"),
    }
    req["gateway_vertex_history"] = json!([{"assistant_turn":0,"turn":turn}]);
    req
}
fn material(calls: &[public_wire::Call]) -> Value {
    let mut index = 0;
    let projected: Vec<_> = parts()
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let mut v = json!({});
            for k in ["text", "thought"] {
                if let Some(raw) = p.get(k) {
                    v[k] = raw.clone();
                }
            }
            if let Some(sig) = p.get("thoughtSignature") {
                v["thought_signature"] = sig.clone();
            }
            if let Some(call) = p.get("functionCall") {
                v["function_call"] = call.clone();
                v["public_id"] = json!(calls[index].id);
                index += 1;
            }
            v
        })
        .collect();
    json!({"source":"vertex_user_oauth","model":"gemini-contract","parts":projected})
}
#[tokio::test]
async fn vertex_oauth_signed_three_protocols_json_sse_preserve_order_state_and_settle() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::sequence(vec![signed_body(), success_body()]).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture
                        .open(endpoint, public_wire::request_body(endpoint, stream))
                        .await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    let calls = public_wire::calls(&text, endpoint, stream);
                    let turn = turn(&text, endpoint, stream);
                    assert_eq!(turn, material(&calls));
                    assert!(!text.contains(ACCESS));
                    assert_eq!(fixture.audit().await.status, "completed");
                    assert_eq!(fixture.remaining().await, 1);
                    let mut req = followup(endpoint, stream, &calls, turn);
                    // Responses does not forward unknown contents; it must remain ignored, not overwrite state.
                    if endpoint == "responses" {
                        req["contents"] =
                            json!([{"role":"model","parts":[{"text":"wrong-override"}]}]);
                    }
                    let response = fixture.open(endpoint, req).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    assert!(text.contains("合成答案"));
                    for row in tools::audits(&fixture, 2).await {
                        assert_eq!(row.status, "completed");
                        assert_identity(&row);
                        assert_eq!(row.total_tokens, Some(5));
                    }
                    assert_eq!(fixture.remaining().await, 0);
                    tools::assert_roundtrip(&server, &calls);
                    let captured = server.calls.lock().unwrap();
                    assert_eq!(captured[1].2["contents"][1]["parts"], parts());
                    assert!(!captured[1].2.to_string().contains("wrong-override"));
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}
