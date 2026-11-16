//! Product guard and transport tests use synthetic material and random loopback only.
use super::*;
use crate::protocol::qwen::workspace_api;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"openai_compatible",
        "auth_mode":workspace_api::AUTH_MODE, "api_key":"synthetic-qwen-key",
        "base_url":"https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1"}))
    .unwrap()
}

#[test]
fn qwen_workspace_never_qualifies_for_native_codex_or_compact() {
    for marker in [true, false] {
        for path in [None, Some("/responses")] {
            let mut payload = payload();
            payload.responses_path = path.map(str::to_owned);
            if !marker {
                payload.auth_mode = Some("bearer".into());
            }
            for stream in [false, true] {
                let req = responses::normalize_responses(json!({"model":"qwen-contract",
                    "input":"hello", "stream":stream}))
                .unwrap();
                assert!(!UpstreamClient::uses_native_codex(
                    &payload,
                    &req,
                    "qwen-contract"
                ));
                assert!(compact::plan(&payload, &req, "qwen-contract").is_err());
            }
        }
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[tokio::test]
async fn qwen_workspace_compact_rejects_before_transport() {
    let req =
        responses::normalize_responses(json!({"model":"qwen-contract","input":"hello"})).unwrap();
    let wire = NativeRequestWire {
        native_session: None,
        compact: true,
        websocket: None,
        connection: None,
        body: bytes::Bytes::from_static(b"{}"),
        query: None,
        client_request_id: false,
        header_order: vec![],
    };
    let result = UpstreamClient::new(1)
        .execute_json_with_wire(
            "qwen-contract",
            ProviderExecutionMode::DirectHttp,
            &payload(),
            &req,
            "qwen-contract",
            None,
            Some(&wire),
        )
        .await;
    assert!(result.unwrap_err().message.contains("Compact requires"));
}

#[cfg(feature = "line-qwen-official-api")]
#[tokio::test]
async fn qwen_workspace_real_sender_keeps_bearer_chat_and_refuses_redirects() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for redirect in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut block = [0; 4096];
                let count = socket.read(&mut block).await.unwrap();
                assert!(count > 0 && bytes.len() + count <= 32768);
                bytes.extend_from_slice(&block[..count]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    let size: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() == end + 4 + size {
                        break;
                    }
                }
            }
            let reply = if redirect {
                format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{address}/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            } else {
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".into()
            };
            socket.write_all(reply.as_bytes()).await.unwrap();
            if redirect {
                assert!(
                    tokio::time::timeout(Duration::from_millis(300), listener.accept())
                        .await
                        .is_err()
                );
            }
            bytes
        });
        let payload = payload();
        let req = responses::normalize_responses(json!({"model":"qwen-contract","input":"hello"}))
            .unwrap();
        let mut plan =
            UpstreamClient::build_request_plan(&payload, &req, "qwen-contract", false).unwrap();
        assert!(plan.url.ends_with("/compatible-mode/v1/chat/completions"));
        // Replace only the transport target after the production descriptor and planner ran.
        plan.url = format!("http://{address}/compatible-mode/v1/chat/completions");
        let client = UpstreamClient::new(2);
        let headers = build_inference_headers(&payload, &req, "qwen-contract", None);
        let response = client
            .send_profiled_plan(&payload, &plan, headers)
            .unwrap()
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), if redirect { 307 } else { 200 });
        let bytes = tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
        assert!(headers.starts_with("post /compatible-mode/v1/chat/completions "));
        assert!(headers.contains("authorization: bearer synthetic-qwen-key"));
        let body: Value = serde_json::from_slice(&bytes[end + 4..]).unwrap();
        assert!(body["messages"].is_array());
        assert!(body.get("input").is_none());
    }
}
