//! Real send-path proof: ambiguous delivery is not replayed; a new request can recover.
use super::*;
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn read_request(socket: &mut tokio::net::TcpStream) {
    let mut request = Vec::new();
    loop {
        let mut buffer = [0; 4096];
        let count = socket.read(&mut buffer).await.unwrap();
        assert!(count > 0, "request ended before its declared body");
        request.extend_from_slice(&buffer[..count]);
        assert!(request.len() <= 16 * 1024, "fixture request exceeded bound");
        let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let header = std::str::from_utf8(&request[..end]).unwrap();
        let length: usize = header
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .map(|value| value.parse().unwrap())
            })
            .unwrap();
        if request.len() >= end + 4 + length {
            return;
        }
    }
}

#[tokio::test]
async fn native_codex_send_failure_retires_transport_without_replaying_business_request() {
    tokio::time::timeout(Duration::from_secs(10), async {
        for stream in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let (quiet_tx, quiet_rx) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                let (mut first, _) = listener.accept().await.unwrap();
                read_request(&mut first).await;
                drop(first);
                // Do not permit the next explicit request until the no-replay check completes.
                assert!(tokio::time::timeout(Duration::from_millis(150), listener.accept()).await.is_err(),
                    "failed request was automatically replayed");
                quiet_tx.send(()).unwrap();
                let (mut second, _) = listener.accept().await.unwrap();
                read_request(&mut second).await;
                let body = b"{\"future_reply\":true}";
                second.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
                second.write_all(body).await.unwrap();
            });
            let payload: ProviderAccountPayload = serde_json::from_value(json!({
                "adapter":"openai_compatible", "base_url":base,
                "api_key":"fixture-only", "request_client_profile":"codex_cli"
            })).unwrap();
            let raw = json!({"model":"gpt-fixture", "input":"hello", "stream":stream}).to_string();
            let req = responses::normalize_responses(serde_json::from_str(&raw).unwrap()).unwrap();
            let connection = crate::http_client::NativeCodexConnection::default();
            let wire = NativeRequestWire {
                native_session: None, compact: false, websocket: None,
                connection: Some(connection.clone()), body: Bytes::from(raw),
                query: None, client_request_id: false, header_order: Vec::new(),
            };
            let client = UpstreamClient::new(3);
            let error = if stream {
                match client.execute_stream_with_wire(&payload, &req, "gpt-fixture", None, Some(&wire)).await {
                    Err(error) => error,
                    Ok(_) => panic!("EOF must fail the original stream request"),
                }
            } else {
                client.execute_json_with_wire("fixture", ProviderExecutionMode::DirectHttp, &payload,
                    &req, "gpt-fixture", None, Some(&wire)).await.unwrap_err()
            };
            assert!(error.message.contains("phase=delivery_unknown"), "{}", error.message);
            assert!(error.native_transport.is_some());
            assert!(!error.retryable);
            assert!(!error.native_connect_retry_allowed());
            assert_eq!(connection.generation(), 1);
            quiet_rx.await.unwrap();
            if stream {
                let UpstreamStreamingResponse::Native(response) = client.execute_stream_with_wire(
                    &payload, &req, "gpt-fixture", None, Some(&wire)).await.unwrap() else {
                    panic!("native request changed transport");
                };
                assert_eq!(response.bytes().await.unwrap().as_ref(), b"{\"future_reply\":true}");
            } else {
                let response = client.execute_json_with_wire("fixture", ProviderExecutionMode::DirectHttp,
                    &payload, &req, "gpt-fixture", None, Some(&wire)).await.unwrap();
                assert_eq!(response["future_reply"], true);
            }
            assert_eq!(connection.generation(), 1);
            server.await.unwrap();
        }
    }).await.unwrap();
}
