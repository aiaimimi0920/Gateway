//! Loopback Realtime control frames; no Redis, database or upstream requests.

use std::time::Duration;

use axum::http::StatusCode;
use futures::{SinkExt, StreamExt};
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::RouteConfigStore;
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

#[path = "console_contract_support/mod.rs"]
mod console_support;
mod support;

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;
const DEADLINE: Duration = Duration::from_secs(3);

struct TestServer {
    task: JoinHandle<()>,
    url: String,
}

impl TestServer {
    async fn start() -> Self {
        let state = support::build_test_app_state(
            console_support::test_config(),
            RouteConfigStore::new(),
            None,
        );
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, build_router(state)).await.unwrap();
        });
        Self {
            task,
            url: format!("ws://{address}/v1/realtime?model=contract-model"),
        }
    }

    async fn connect(&self) -> Client {
        let (client, response) = timeout(DEADLINE, connect_async(&self.url))
            .await
            .expect("WebSocket upgrade timed out")
            .unwrap();
        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
        client
    }

    async fn stop(mut self) {
        self.task.abort();
        assert!((&mut self.task).await.unwrap_err().is_cancelled());
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Assertion failures must release the loopback listener too.
        self.task.abort();
    }
}

async fn next_frame(client: &mut Client) -> Message {
    timeout(DEADLINE, client.next())
        .await
        .expect("WebSocket frame timed out")
        .expect("WebSocket closed before the expected frame")
        .unwrap()
}

async fn next_json(client: &mut Client) -> Value {
    let Message::Text(text) = next_frame(client).await else {
        panic!("Expected a JSON text frame");
    };
    serde_json::from_str(&text).unwrap()
}

async fn send_frame(client: &mut Client, frame: Message) {
    timeout(DEADLINE, client.send(frame))
        .await
        .expect("WebSocket send timed out")
        .unwrap();
}

async fn send_json(client: &mut Client, value: Value) {
    send_frame(client, Message::Text(value.to_string().into())).await;
}

async fn close_client(mut client: Client) {
    timeout(DEADLINE, client.close(None))
        .await
        .expect("WebSocket close timed out")
        .unwrap();
}

#[tokio::test]
async fn session_and_conversation_control_frames_preserve_identity_and_order() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    let created = next_json(&mut client).await;
    assert_eq!(created["type"], "session.created");
    assert_eq!(created["session"]["model"], "contract-model");
    assert!(created["session"]["id"]
        .as_str()
        .unwrap()
        .starts_with("sess_"));

    send_json(&mut client, json!({
        "type": "session.update", "session": {"model": "updated-model", "instructions": "Synthetic instructions"}
    })).await;
    let updated = next_json(&mut client).await;
    assert_eq!(updated["type"], "session.updated");
    assert_eq!(updated["session"]["id"], created["session"]["id"]);
    assert_eq!(updated["session"]["model"], "updated-model");

    let item = json!({"id": "client-item", "type": "message", "role": "user", "content": [{"input_text": "Question"}]});
    send_json(
        &mut client,
        json!({"type": "conversation.item.create", "item": item}),
    )
    .await;
    let acknowledged = next_json(&mut client).await;
    assert_eq!(acknowledged["type"], "conversation.item.created");
    assert_eq!(acknowledged["item"], item);

    send_json(&mut client, json!({"type": "ping"})).await;
    assert_eq!(next_json(&mut client).await, json!({"type": "pong"}));
    send_frame(&mut client, Message::Ping(vec![1, 2, 3].into())).await;
    assert_eq!(
        next_frame(&mut client).await,
        Message::Pong(vec![1, 2, 3].into())
    );

    close_client(client).await;
    server.stop().await;
}

#[tokio::test]
async fn invalid_and_unsupported_frames_return_errors_without_closing_session() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    assert_eq!(next_json(&mut client).await["type"], "session.created");

    for (frame, message) in [
        (Message::Text("{".into()), "invalid JSON frame"),
        (
            Message::Binary(vec![1, 2].into()),
            "binary websocket frames",
        ),
        (
            Message::Text(
                json!({"type": "input_audio_buffer.append"})
                    .to_string()
                    .into(),
            ),
            "audio buffers are not supported",
        ),
        (
            Message::Text(json!({"type": "unknown.event"}).to_string().into()),
            "unsupported realtime event",
        ),
    ] {
        send_frame(&mut client, frame).await;
        let error = next_json(&mut client).await;
        assert_eq!(error["type"], "error");
        assert!(error["error"]["message"]
            .as_str()
            .unwrap()
            .contains(message));
        send_json(&mut client, json!({"type": "ping"})).await;
        assert_eq!(next_json(&mut client).await, json!({"type": "pong"}));
    }

    close_client(client).await;
    server.stop().await;
}
