//! Failure/cancellation cases must not consume a prepaid message or leak a replay.
use super::*;

async fn connect(
    fixture: &Fixture,
    token: &str,
    session: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let mut request = (fixture.base.replacen("http", "ws", 1) + "/v1/responses")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    request
        .headers_mut()
        .insert("session-id", session.parse().unwrap());
    tokio_tungstenite::connect_async(request).await.unwrap().0
}

#[tokio::test]
async fn codex_ws_errors_ping_and_cancellation_refund() {
    Box::pin(async {
        let fixture = Fixture::start().await;
        let (id, token) = fixture.key(3).await;
        let mut socket = connect(&fixture, &token, "failure-test").await;
        for malformed in [
            r#"{"type":"response.create","model":"gpt-fixture","model":"other","input":[]}"#,
            r#"{"type":"session.update"}"#,
        ] {
            socket.send(Message::Text(malformed.into())).await.unwrap();
            assert_eq!(event(&mut socket).await["type"], "error");
        }
        assert!(fixture.capture.lock().unwrap().frames.is_empty());
        socket
            .send(Message::Text(
                json!({"type":"response.create","model":"gpt-fixture","input":"fail"}).to_string(),
            ))
            .await
            .unwrap();
        let failure = event(&mut socket).await;
        assert_eq!(failure["error"]["code"], "fixture_failed");
        fixture.wait_balance(&id, 3).await;
        socket
            .send(Message::Text(
                json!({"type":"response.create","model":"gpt-fixture","input":"wait"}).to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(event(&mut socket).await["type"], "response.created");
        socket.send(Message::Ping(vec![1, 2, 3])).await.unwrap();
        let pong = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(pong, Message::Pong(vec![1, 2, 3]));
        socket.close(None).await.unwrap();
        fixture.wait_balance(&id, 3).await;
        assert_eq!(fixture.capture.lock().unwrap().frames.len(), 2);
        fixture.stop().await;
    })
    .await;
}

#[tokio::test]
async fn codex_ws_disconnect_during_upgrade_refunds_before_stream_handoff() {
    Box::pin(async {
        let fixture = Fixture::start().await;
        let (id, token) = fixture.key(2).await;
        let mut socket = connect(&fixture, &token, "slow-handshake").await;
        socket
            .send(Message::Text(
                json!({"type":"response.create","model":"gpt-fixture","input":[]}).to_string(),
            ))
            .await
            .unwrap();
        for _ in 0..100 {
            if fixture.capture.lock().unwrap().connections == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(fixture.capture.lock().unwrap().connections, 1);
        socket.close(None).await.unwrap();
        fixture.wait_balance(&id, 2).await;
        assert!(fixture.capture.lock().unwrap().frames.is_empty());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        fixture.stop().await;
    })
    .await;
}

async fn event(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    let message = tokio::time::timeout(std::time::Duration::from_secs(10), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(message.to_text().unwrap()).unwrap()
}
