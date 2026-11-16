//! Real loopback bodies prove that partial Qwen text is not a successful response.
#![cfg(feature = "line-qwen-web-reverse")]
use neuro_gateway::{error::GatewayError, protocol::canonical::CanonicalRelayResponse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn collect(body: &'static str) -> Result<CanonicalRelayResponse, GatewayError> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 4096];
        socket.read(&mut request).await.unwrap();
        let header = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        socket.write_all(header.as_bytes()).await.unwrap();
        socket.write_all(body.as_bytes()).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    let response = rquest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap()
        .get(format!("http://{address}/synthetic"))
        .send()
        .await
        .unwrap();
    let result =
        neuro_gateway::protocol::qwen_web::accumulate_qwen_web_stream(response, "synthetic-model")
            .await;
    peer.await.unwrap();
    result
}

#[tokio::test]
async fn qwen_web_partial_eof_is_not_success() {
    assert!(collect(
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\",\"phase\":\"answer\"}}]}\n\n"
    )
    .await
    .is_err());
}

#[tokio::test]
async fn qwen_web_error_after_text_is_not_success() {
    assert!(collect(concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\",\"phase\":\"answer\"}}]}\n\n",
        "data: {\"error\":{\"code\":\"synthetic_failure\"},\"done\":true}\n\n",
        "data: [DONE]\n\n"
    ))
    .await
    .is_err());
}

#[tokio::test]
async fn qwen_web_malformed_tail_is_not_success() {
    assert!(collect(concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\",\"phase\":\"answer\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: {bad-json}\n\n"
    )).await.is_err());
}
