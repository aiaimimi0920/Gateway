//! Opaque Google state traverses real public JSON/SSE, then the next exact Express request.
use super::tools::public_wire;
use super::*;
#[path = "vertex_express_pipeline_signed_failures.rs"]
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
        json!({"candidates":[{"content":{"parts":parts()},"finishReason":"STOP"}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}}),
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
        "messages" => {
            req["messages"][1]["content"]
                .as_array_mut()
                .unwrap()
                .insert(0, json!({"type":"text","text":"AB"}));
        }
        "responses" => {
            req["input"].as_array_mut().unwrap().insert(
                1,
                json!({"role":"assistant","content":[{"type":"output_text","text":"AB"}]}),
            );
        }
        _ => {
            req["messages"][1]["content"] = json!("AB");
        }
    }
    req["gateway_vertex_history"] = json!([{"assistant_turn":0,"turn":turn}]);
    req
}

#[tokio::test]
async fn vertex_express_signed_three_protocols_json_sse_roundtrip_and_settle() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::sequence(vec![signed_body(), success_body()]).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture
                        .open(endpoint, public_wire::request_body(endpoint, stream))
                        .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let text = drain(response.into_body()).await;
                    let calls = public_wire::calls(&text, endpoint, stream);
                    let turn = turn(&text, endpoint, stream);
                    assert_eq!(turn["parts"].as_array().expect("typed parts").len(), 6);
                    assert_eq!(fixture.audit().await.status, "completed");
                    assert_eq!(fixture.remaining().await, 1);
                    let response = fixture
                        .open(endpoint, followup(endpoint, stream, &calls, turn))
                        .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    assert!(drain(response.into_body()).await.contains("合成答案"));
                    assert_eq!(super::tools::audits(&fixture, 2).await.len(), 2);
                    assert_eq!(fixture.remaining().await, 0);
                    super::tools::assert_roundtrip_wire(&server, &calls);
                    let captured = server.calls.lock().unwrap();
                    assert_eq!(captured[1].2["contents"][1]["parts"], parts());
                    assert!(captured[1].2.get("gateway_vertex_history").is_none());
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}
