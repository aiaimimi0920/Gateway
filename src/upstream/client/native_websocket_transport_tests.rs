//! Real compressed frames over bounded in-memory IO, without external services.
use futures::{SinkExt, StreamExt};
use tokio_tungstenite_native::WebSocketStream;
use tungstenite_native::{
    extensions::compression::deflate::DeflateConfig,
    protocol::{Role, WebSocketConfig},
    Message,
};

fn config(compression: bool, limit: usize) -> WebSocketConfig {
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(limit);
    config.max_frame_size = Some(limit);
    config.extensions.permessage_deflate = compression.then(DeflateConfig::default);
    config
}

#[tokio::test]
async fn native_websocket_compression_preserves_context_across_messages() {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let (left, right) = tokio::io::duplex(8192);
        let mut client =
            WebSocketStream::from_raw_socket(left, Role::Client, Some(config(true, 65536))).await;
        let mut server =
            WebSocketStream::from_raw_socket(right, Role::Server, Some(config(true, 65536))).await;
        let text = "opaque encrypted response repeat ".repeat(512);
        for _ in 0..2 {
            let (sent, received) = tokio::join!(
                server.send(Message::Text(text.clone().into())),
                client.next()
            );
            sent.unwrap();
            assert_eq!(received.unwrap().unwrap().into_text().unwrap(), text);
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn native_websocket_rejects_bombs_and_unnegotiated_compression() {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        for compression in [false, true] {
            let (left, right) = tokio::io::duplex(8192);
            let mut client = WebSocketStream::from_raw_socket(
                left,
                Role::Client,
                Some(config(compression, 4096)),
            )
            .await;
            let mut server =
                WebSocketStream::from_raw_socket(right, Role::Server, Some(config(true, 65536)))
                    .await;
            let (sent, received) = tokio::join!(
                server.send(Message::Text("x".repeat(32768).into())),
                client.next()
            );
            sent.unwrap();
            assert!(received.unwrap().is_err(), "compression={compression}");
        }
    })
    .await
    .unwrap();
}
