//! Bounded one-shot loopback proves format -> identity -> transport, without live keys.
use super::*;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn receiver(streaming: bool) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(8), async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut block = [0; 2048];
                let n = socket.read(&mut block).await.unwrap();
                assert!(n > 0 && bytes.len() + n <= 32768);
                bytes.extend_from_slice(&block[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    let length = headers.lines().find_map(|l| l.strip_prefix("content-length: "))
                        .unwrap().parse::<usize>().unwrap();
                    if bytes.len() == end + 4 + length { break; }
                }
            }
            let (kind, body) = if streaming {
                ("text/event-stream", concat!(
                    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"fixture\",\"model\":\"fixture\",\"usage\":{\"input_tokens\":1}}}\n\n",
                    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"OK\"}}\n\n",
                    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
                    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
                ))
            } else {
                ("application/json", r#"{"id":"fixture","type":"message","role":"assistant","model":"fixture","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}"#)
            };
            let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(body.as_bytes()).await.unwrap();
            bytes
        }).await.expect("bounded receiver")
    });
    (base, task)
}

#[tokio::test]
async fn call_flow_wire_covers_buffered_stream_passthrough_and_native_shortcuts() {
    for case in 0..12 {
        let lane = case % 6;
        let streaming = matches!(lane, 1 | 3 | 4);
        // Buffered Anthropic execution intentionally accumulates an upstream SSE stream.
        let upstream_streaming = streaming || lane == 0;
        let (base, server) = receiver(upstream_streaming).await;
        let (base, expected_path) = if case >= 6 {
            (
                format!("{base}/api.openai.com"),
                "/api.openai.com/v1/messages",
            )
        } else {
            (base, "/v1/messages")
        };
        // Deliberately stale legacy adapter/profile: flow must resolve before any shortcut.
        let payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"openai_compatible", "base_url":base, "api_key":"selected-secret",
            "request_client_profile":"codex_cli",
            "call_flow":{"call_adapter":"anthropic","fingerprint_adapter":"claude_cli","special_adapter":"none"}
        })).unwrap();
        let req = if lane == 4 {
            crate::protocol::anthropic::normalize_messages(json!({"model":"fixture","stream":true,"max_tokens":8,"messages":[{"role":"user","content":"hello"}]})).unwrap()
        } else {
            responses::normalize_responses(
                json!({"model":"fixture","stream":streaming,"input":"hello"}),
            )
            .unwrap()
        };
        let client = UpstreamClient::new(5);
        let result = tokio::time::timeout(Duration::from_secs(7), async {
            match lane {
                0 => {
                    let reply = client.execute(&payload, &req, "fixture", None).await?;
                    assert_eq!(reply.text, "OK");
                }
                1 | 4 => {
                    client
                        .execute_stream(&payload, &req, "fixture", None)
                        .await?;
                }
                2 => {
                    client
                        .execute_json_passthrough(
                            "fixture",
                            ProviderExecutionMode::DirectHttp,
                            &payload,
                            &req,
                            "fixture",
                            None,
                        )
                        .await?;
                }
                3 => {
                    client
                        .execute_stream_with_wire(&payload, &req, "fixture", None, None)
                        .await?;
                }
                _ => {
                    client
                        .execute_json_with_wire(
                            "fixture",
                            ProviderExecutionMode::DirectHttp,
                            &payload,
                            &req,
                            "fixture",
                            None,
                            None,
                        )
                        .await?;
                }
            }
            Ok::<_, GatewayError>(())
        })
        .await;
        let captured = server.await.unwrap();
        result.expect("bounded upstream").unwrap();
        let end = captured.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let headers = String::from_utf8_lossy(&captured[..end]).to_ascii_lowercase();
        assert!(
            headers.starts_with(&format!("post {expected_path} http/1.1")),
            "lane={lane}: {headers}"
        );
        assert!(headers.contains("x-api-key: selected-secret"));
        assert!(headers.contains(&format!(
            "user-agent: {}",
            crate::protocol::claude_wire::USER_AGENT.to_ascii_lowercase()
        )));
        let body: Value = serde_json::from_slice(&captured[end + 4..]).unwrap();
        assert!(body["messages"].is_array());
        assert_eq!(body["model"], "fixture");
        assert_eq!(body["stream"], upstream_streaming, "lane={lane}");
        assert!(body.get("call_flow").is_none());
        assert!(!String::from_utf8_lossy(&captured[end + 4..]).contains("selected-secret"));
    }
}

#[tokio::test]
async fn call_flow_invalid_combinations_fail_before_any_network_or_session_work() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "base_url":"http://127.0.0.1:1", "api_key":"fixture",
        "call_flow":{"special_adapter":"chatgpt_web"}
    }))
    .unwrap();
    let req = responses::normalize_responses(json!({"model":"fixture","input":"hello"})).unwrap();
    let client = UpstreamClient::new(1);
    for lane in 0..4 {
        let error = match lane {
            0 => client
                .execute(&payload, &req, "fixture", None)
                .await
                .err()
                .unwrap(),
            1 => client
                .execute_stream(&payload, &req, "fixture", None)
                .await
                .err()
                .unwrap(),
            2 => client
                .execute_json_passthrough(
                    "fixture",
                    ProviderExecutionMode::DirectHttp,
                    &payload,
                    &req,
                    "fixture",
                    None,
                )
                .await
                .err()
                .unwrap(),
            _ => client
                .execute_binary_passthrough(&payload, &req, "fixture", None)
                .await
                .err()
                .unwrap(),
        };
        assert!(error.to_string().contains("Invalid call_flow"));
    }
}
