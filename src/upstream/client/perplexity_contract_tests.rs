//! Perplexity 官方合同：假密钥、有界 loopback；不调用真实服务商。
use super::*;
use crate::preset::{compile_provider_account, get_builtin_preset, AccountOverrides};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn payload(base: &str) -> ProviderAccountPayload {
    let preset = get_builtin_preset("perplexity-api").expect("Perplexity preset");
    let account: AccountOverrides = serde_json::from_value(json!({
        "base_url":base, "api_key":"perplexity-fixture-secret"
    }))
    .unwrap();
    compile_provider_account(&preset, &account)
}

fn request(stream: bool) -> CanonicalRelayRequest {
    openai::normalize_chat_completions(json!({"model":"sonar", "stream":stream,
        "max_tokens":16,
        "messages":[{"role":"user","content":"Reply OK"}]}))
    .unwrap()
}

#[test]
fn perplexity_contract_registry_and_official_url() {
    assert!(get_builtin_preset("perplexity").is_none());
    assert_eq!(
        crate::protocol::registry::default_protocol_profile_for_preset("perplexity-api"),
        "perplexity_chat"
    );
    for base in ["https://api.perplexity.ai", "https://api.perplexity.ai/"] {
        for streaming in [false, true] {
            let plan = UpstreamClient::build_request_plan(
                &payload(base),
                &request(streaming),
                "sonar",
                streaming,
            )
            .unwrap();
            assert_eq!(plan.url, "https://api.perplexity.ai/v1/sonar");
            let body = plan.body.unwrap();
            assert_eq!(body["stream"], streaming);
        }
    }
}

async fn receiver(stream: bool, unauthorized: bool) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
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
                    let length = headers.lines().find_map(|l| l.strip_prefix("content-length: ")).unwrap().parse::<usize>().unwrap();
                    if bytes.len() == end + 4 + length { break; }
                }
            }
            let (status, kind, body) = if unauthorized {
                ("401 Unauthorized", "application/json", r#"{"error":{"message":"Invalid API key provided.","type":"invalid_api_key","code":401}}"#)
            } else if stream {
                ("200 OK", "text/event-stream", concat!(
                    ": keep-alive\n\n",
                    "data: {\"id\":\"fixture\",\"object\":\"chat.completion.chunk\",\"model\":\"sonar\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\"},\"finish_reason\":null}]}\n\n",
                    "data: {\"id\":\"fixture\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                    "data: [DONE]\n\n"))
            } else {
                ("200 OK", "application/json", r#"{"id":"fixture","object":"chat.completion","model":"sonar","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}"#)
            };
            let headers = format!("HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(body.as_bytes()).await.unwrap();
            bytes
        }).await.expect("bounded receiver")
    });
    (base, task)
}

#[tokio::test]
async fn perplexity_contract_wire_json_sse_and_unauthorized() {
    for (stream, unauthorized) in [(false, false), (true, false), (false, true), (true, true)] {
        let (base, server) = receiver(stream, unauthorized).await;
        let client = UpstreamClient::new(5);
        let payload = payload(&base);
        let req = request(stream);
        tokio::time::timeout(Duration::from_secs(7), async {
            if stream {
                let result = client.execute_stream(&payload, &req, "sonar", None).await;
                if unauthorized {
                    assert!(result.is_err());
                } else {
                    let UpstreamStreamingResponse::Http(response) = result.unwrap() else {
                        panic!("expected HTTP SSE")
                    };
                    assert!(response.headers()["content-type"]
                        .to_str()
                        .unwrap()
                        .contains("text/event-stream"));
                    let body = response.text().await.unwrap();
                    assert!(body.contains("\"content\":\"OK\""));
                    assert!(body.ends_with("data: [DONE]\n\n"));
                }
            } else {
                let result = client.execute(&payload, &req, "sonar", None).await;
                if unauthorized {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().text, "OK");
                }
            }
        })
        .await
        .expect("bounded request");
        let captured = server.await.unwrap();
        let end = captured.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let headers = String::from_utf8_lossy(&captured[..end]).to_ascii_lowercase();
        assert!(headers.starts_with("post /v1/sonar http/1.1"));
        assert!(headers.contains("authorization: bearer perplexity-fixture-secret"));
        assert!(!headers.contains("x-api-key:"));
        let body: Value = serde_json::from_slice(&captured[end + 4..]).unwrap();
        assert_eq!(body["model"], "sonar");
        assert_eq!(body["stream"], stream);
        assert_eq!(body["messages"][0]["content"], "Reply OK");
        assert!(!body.to_string().contains("perplexity-fixture-secret"));
    }
}
