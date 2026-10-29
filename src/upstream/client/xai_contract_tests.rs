//! xAI 文档合同：假密钥、有界本地 HTTP 接收端，不请求真实上游。
use super::*;
use crate::preset::{compile_provider_account, get_builtin_preset, AccountOverrides};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn payload(base: &str) -> ProviderAccountPayload {
    let account: AccountOverrides = serde_json::from_value(json!({
        "base_url":base, "api_key":"xai-fixture-secret"
    }))
    .unwrap();
    compile_provider_account(&get_builtin_preset("xai-api").unwrap(), &account)
}

fn request(stream: bool, responses_api: bool) -> CanonicalRelayRequest {
    if responses_api {
        responses::normalize_responses(
            json!({"model":"grok-4.7","input":"Reply OK","stream":stream,"store":false}),
        )
        .unwrap()
    } else {
        openai::normalize_chat_completions(json!({"model":"grok-4.7","stream":stream,
            "messages":[{"role":"user","content":"Reply OK"}]}))
        .unwrap()
    }
}

fn path(responses_api: bool) -> &'static str {
    if responses_api {
        "/v1/responses"
    } else {
        "/v1/chat/completions"
    }
}

#[test]
fn xai_contract_registry_and_official_urls() {
    assert!(get_builtin_preset("xai-openai").is_none());
    assert_eq!(
        crate::protocol::registry::default_protocol_profile_for_preset("xai-api"),
        "xai"
    );
    for base in ["https://api.x.ai/v1", "https://api.x.ai/v1/"] {
        for responses_api in [false, true] {
            for streaming in [false, true] {
                let plan = UpstreamClient::build_request_plan(
                    &payload(base),
                    &request(streaming, responses_api),
                    "grok-4.7",
                    streaming,
                )
                .unwrap();
                assert_eq!(plan.url, format!("https://api.x.ai{}", path(responses_api)));
                assert_eq!(plan.body.unwrap()["stream"], streaming);
            }
        }
    }
}

fn reply(streaming: bool, responses_api: bool) -> String {
    let response = if responses_api {
        json!({"id":"resp_fixture","object":"response","status":"completed","model":"grok-4.7",
            "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK"}]}],
            "usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}})
    } else {
        json!({"id":"fixture","object":"chat.completion","model":"grok-4.7",
            "choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}})
    };
    if !streaming {
        return response.to_string();
    }
    if responses_api {
        format!("event: response.output_text.delta\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
            json!({"type":"response.output_text.delta","delta":"OK","output_index":0,"content_index":0}),
            json!({"type":"response.completed","response":response}))
    } else {
        format!(
            "data: {}\n\ndata: [DONE]\n\n",
            json!({"id":"fixture","object":"chat.completion.chunk","model":"grok-4.7",
            "choices":[{"index":0,"delta":{"content":"OK"},"finish_reason":"stop"}]})
        )
    }
}

async fn receiver(
    streaming: bool,
    responses_api: bool,
    unauthorized: bool,
) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
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
                ("401 Unauthorized", "application/json", json!({"error":"Unauthorized"}).to_string())
            } else {
                ("200 OK", if streaming { "text/event-stream" } else { "application/json" }, reply(streaming, responses_api))
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
async fn xai_contract_wire_chat_and_responses_json_sse_auth_errors() {
    for responses_api in [false, true] {
        for (streaming, unauthorized) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let (base, server) = receiver(streaming, responses_api, unauthorized).await;
            let client = UpstreamClient::new(5);
            let payload = payload(&base);
            let req = request(streaming, responses_api);
            tokio::time::timeout(Duration::from_secs(7), async {
                if streaming {
                    let result = client
                        .execute_stream(&payload, &req, "grok-4.7", None)
                        .await;
                    if unauthorized {
                        assert!(result.is_err());
                    } else {
                        let UpstreamStreamingResponse::Http(response) = result.unwrap() else {
                            panic!("expected HTTP SSE");
                        };
                        assert!(response.headers()["content-type"]
                            .to_str()
                            .unwrap()
                            .contains("text/event-stream"));
                        assert_eq!(response.text().await.unwrap(), reply(true, responses_api));
                    }
                } else {
                    let result = client.execute(&payload, &req, "grok-4.7", None).await;
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
            assert!(headers.starts_with(&format!("post {} http/1.1", path(responses_api))));
            assert!(headers.contains("authorization: bearer xai-fixture-secret"));
            assert!(!headers.contains("x-api-key:"));
            let body: Value = serde_json::from_slice(&captured[end + 4..]).unwrap();
            assert_eq!(body["model"], "grok-4.7");
            assert_eq!(body["stream"], streaming);
            if responses_api {
                assert_eq!(body["input"], "Reply OK");
                assert_eq!(body["store"], false);
            } else {
                assert_eq!(body["messages"][0]["content"], "Reply OK");
            }
            assert!(!body.to_string().contains("xai-fixture-secret"));
        }
    }
}
