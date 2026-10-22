//! Real loopback HTTP/WS against the local-mode router; no provider credentials.
#[path = "codex_application_contract/failures.rs"]
mod failures;
#[path = "codex_application_contract/fixture.rs"]
mod fixture;
#[path = "console_contract_support/mod.rs"]
mod support;
use fixture::*;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

#[tokio::test]
async fn codex_compact_and_serial_websocket_preserve_governance() {
    Box::pin(flow()).await;
}

async fn flow() {
    let fixture = Fixture::start().await;
    let (key_id, token) = fixture.key(4).await;
    let raw = "{ \"model\":\"gpt-fixture\", \"input\":[], \"instructions\":\"compact\" }";
    let reply = fixture
        .http
        .post(format!("{}/v1/responses/compact", fixture.base))
        .bearer_auth(&token)
        .header("content-type", "application/json")
        .body(raw)
        .send()
        .await
        .unwrap();
    assert!(
        reply.status().is_success(),
        "{}",
        reply.text().await.unwrap()
    );
    let reply: Value = reply.json().await.unwrap();
    assert_eq!(reply["object"], "response.compaction");
    assert_eq!(reply["output"][0]["encrypted_content"], "opaque-fixture");
    assert_eq!(fixture.balance(&key_id).await, 3);
    assert_eq!(fixture.capture.lock().unwrap().compact[0], raw);

    let url = fixture.base.replacen("http", "ws", 1) + "/v1/responses";
    let mut unauthorized = url.clone().into_client_request().unwrap();
    unauthorized
        .headers_mut()
        .insert("authorization", "Bearer invalid".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(unauthorized)
        .await
        .is_err());
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    for turn in 0..2 {
        let body = json!({"type":"response.create","model":"gpt-fixture","input":[],"stream":true,
            "instructions":"native","store":false,"include":["reasoning.encrypted_content"],
            "parallel_tool_calls":true,"previous_response_id":if turn==0 {Value::Null} else {json!("resp-fixture")},
            "generate":turn != 0, "future_field":{"opaque":true}}).to_string();
        socket.send(Message::Text(body.clone())).await.unwrap();
        let reply = tokio::time::timeout(std::time::Duration::from_secs(10), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let event: Value = serde_json::from_str(reply.to_text().unwrap()).unwrap();
        assert_eq!(event["type"], "response.completed", "{event}");
        assert_eq!(
            event["response"]["output"][0]["encrypted_content"],
            "opaque-fixture"
        );
        assert_eq!(fixture.capture.lock().unwrap().frames[turn as usize], body);
    }
    fixture.wait_balance(&key_id, 1).await;
    assert_eq!(fixture.capture.lock().unwrap().connections, 1);
    for headers in &fixture.capture.lock().unwrap().ws_headers {
        assert!(!headers.contains_key("content-type"));
        assert!(!headers.contains_key("accept"));
    }
    assert!(fixture
        .capture
        .lock()
        .unwrap()
        .auth
        .iter()
        .all(|value| value == "Bearer selected-fixture"));
    // Revoke credit while keeping the same authenticated socket: next create
    // must re-enter admission rather than inherit the handshake's permission.
    fixture.adjust(&key_id, 0).await;
    socket
        .send(Message::Text(
            json!({"type":"response.create","model":"gpt-fixture","input":[]}).to_string(),
        ))
        .await
        .unwrap();
    let rejected = socket.next().await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(rejected.to_text().unwrap()).unwrap()["type"],
        "error"
    );
    assert_eq!(fixture.capture.lock().unwrap().frames.len(), 2);
    socket.close(None).await.unwrap();
    fixture.stop().await;
}
