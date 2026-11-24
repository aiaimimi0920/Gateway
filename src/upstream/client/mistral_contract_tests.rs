//! Mistral 文档合同：仅使用有界 loopback 与假密钥，不请求真实服务商。
use super::*;
use crate::preset::{compile_provider_account, get_builtin_preset, AccountOverrides};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[path = "../../../tests/console_contract_support/mod.rs"]
mod console_support;

fn payload(preset: &str, base: &str) -> ProviderAccountPayload {
    let preset = get_builtin_preset(preset).expect("Mistral preset");
    let account: AccountOverrides = serde_json::from_value(json!({
        "base_url":base, "api_key":"mistral-fixture-secret"
    }))
    .unwrap();
    compile_provider_account(&preset, &account)
}

fn request(stream: bool) -> CanonicalRelayRequest {
    openai::normalize_chat_completions(json!({"model":"mistral-large-latest", "stream":stream,
        "max_tokens":8, "messages":[{"role":"user","content":"Reply OK"}]}))
    .unwrap()
}

#[test]
fn mistral_contract_registry_and_official_url() {
    assert!(get_builtin_preset("mistral-openai").is_none());
    for preset in ["mistral-api", "mistral-studio-api", "mistral-vibe-api"] {
        assert_eq!(
            crate::protocol::registry::default_protocol_profile_for_preset(preset),
            "mistral"
        );
        for base in ["https://api.mistral.ai/v1", "https://api.mistral.ai/v1/"] {
            let payload = payload(preset, base);
            for streaming in [false, true] {
                let req = request(streaming);
                let plan = UpstreamClient::build_request_plan(
                    &payload,
                    &req,
                    "mistral-large-latest",
                    streaming,
                )
                .unwrap();
                let headers = crate::upstream::headers::build_inference_headers(
                    &payload,
                    &req,
                    "mistral-large-latest",
                    None,
                );
                assert_eq!(plan.url, "https://api.mistral.ai/v1/chat/completions");
                let body = plan.body.unwrap();
                assert_eq!(body["stream"], streaming);
                assert_eq!(headers["authorization"], "Bearer mistral-fixture-secret");
                assert_eq!(
                    headers
                        .get("user-agent")
                        .map(|value| value.to_str().unwrap()),
                    (preset == "mistral-vibe-api")
                        .then_some("mistral-client-python/Mistral-Vibe/3.1.4")
                );
                if preset == "mistral-vibe-api" {
                    assert!(body.get("stream_options").is_none());
                    assert_eq!(
                        headers["accept"],
                        if streaming {
                            "text/event-stream"
                        } else {
                            "application/json"
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn mistral_contract_vibe_preserves_explicit_stream_options() {
    let payload = payload("mistral-vibe-api", "https://api.mistral.ai/v1");
    let mut req = request(true);
    let options = json!({"include_usage": true});
    req.extra
        .insert("stream_options".to_string(), options.clone());
    let plan =
        UpstreamClient::build_request_plan(&payload, &req, "mistral-large-latest", true).unwrap();
    assert_eq!(plan.body.unwrap()["stream_options"], options);
}

#[test]
fn mistral_contract_vibe_responses_uses_official_chat_bridge() {
    let vibe = payload("mistral-vibe-api", "https://api.mistral.ai/v1");
    let req = crate::protocol::responses::normalize_responses(json!({
        "model":"mistral-large-latest", "stream":true,
        "instructions":"fixture instructions", "input":"Reply OK"
    }))
    .unwrap();
    let plan =
        UpstreamClient::build_request_plan(&vibe, &req, "mistral-large-latest", true).unwrap();
    assert_eq!(plan.url, "https://api.mistral.ai/v1/chat/completions");
    let body = plan.body.unwrap();
    assert!(body["messages"].is_array());
    assert!(body.get("stream_options").is_none());
    assert!(!body.to_string().contains("session_id"));
}

#[test]
fn mistral_contract_vibe_affinity_uses_session_and_stays_on_official_chat() {
    use crate::upstream::openai_compatible_common::finalize_vibe_affinity;
    let vibe = payload("mistral-vibe-api", "https://api.mistral.ai/v1");
    let studio = payload("mistral-studio-api", "https://api.mistral.ai/v1");
    let root = "e48c3aac-c015-fac8-fe09-391e084e9c63";
    let child = "c5ca2b60-93e9-4258-4914-d676002aeef2";
    let mut plan =
        UpstreamClient::build_request_plan(&vibe, &request(true), "mistral-large-latest", true)
            .unwrap();
    let mut headers = rquest::header::HeaderMap::new();
    finalize_vibe_affinity(&vibe, &plan, &mut headers);
    assert!(!headers.contains_key("x-affinity"));

    plan.body.as_mut().unwrap()["metadata"] = json!({"session_id": child});
    finalize_vibe_affinity(&studio, &plan, &mut headers);
    assert!(!headers.contains_key("x-affinity"));
    finalize_vibe_affinity(&vibe, &plan, &mut headers);
    assert_eq!(headers["x-affinity"], child);
    let context_key = crate::http::request_headers::VIBE_AFFINITY_CONTEXT;
    let req = request(true);
    let mut ctx = crate::pipeline::PipelineContext::new(request(true), None);
    let mut inbound = axum::http::HeaderMap::new();
    inbound.insert("x-affinity", axum::http::HeaderValue::from_static(root));
    crate::http::request_headers::capture_vibe_affinity(&mut ctx, &inbound);
    assert_eq!(ctx.request_headers[context_key], root);
    let build = |payload| {
        crate::upstream::headers::build_inference_headers(
            payload,
            &req,
            "mistral-large-latest",
            Some(&ctx.request_headers),
        )
    };
    let studio_headers = build(&studio);
    assert!(!studio_headers.contains_key("x-affinity"));
    assert!(!studio_headers.contains_key(context_key));
    headers = build(&vibe);
    assert!(!headers.contains_key(context_key));
    finalize_vibe_affinity(&vibe, &plan, &mut headers);
    assert_eq!(headers["x-affinity"], root);

    headers.clear();
    plan.body.as_mut().unwrap()["metadata"] = json!({"session_id": child, "root_session_id": root});
    finalize_vibe_affinity(&vibe, &plan, &mut headers);
    assert_eq!(headers["x-affinity"], root);

    plan.url = "https://api.mistral.ai.example.org/v1/chat/completions".to_owned();
    finalize_vibe_affinity(&vibe, &plan, &mut headers);
    assert!(!headers.contains_key("x-affinity"));

    plan.url = "https://api.mistral.ai/v1/chat/completions".to_owned();
    plan.body.as_mut().unwrap()["metadata"] = json!({"session_id": "not a session UUID"});
    finalize_vibe_affinity(&vibe, &plan, &mut headers);
    assert!(!headers.contains_key("x-affinity"));
}

#[tokio::test]
#[ignore = "manual comparison against a local Vibe CLI capture and non-forwarding TLS fixture"]
async fn mistral_contract_replay_captured_cli_request() {
    let client = wire_fixture_client();
    let body = std::fs::read(std::env::var_os("MISTRAL_WIRE_FIXTURE_BODY").unwrap()).unwrap();
    let input: Value = serde_json::from_slice(&body).unwrap();
    let model = input["model"].as_str().unwrap().to_owned();
    let req = openai::normalize_chat_completions(input).unwrap();
    assert!(req.stream);
    let payload = payload("mistral-vibe-api", "https://api.mistral.ai/v1");
    let response = client
        .execute_stream(&payload, &req, &model, None)
        .await
        .unwrap();
    let UpstreamStreamingResponse::Http(response) = response else {
        panic!("expected HTTP SSE");
    };
    assert!(response.text().await.unwrap().contains("data: [DONE]"));
}

fn wire_fixture_client() -> UpstreamClient {
    let proxy = std::env::var("MISTRAL_WIRE_FIXTURE_PROXY").unwrap();
    let address: std::net::SocketAddrV4 = proxy.parse().unwrap();
    assert!(address.ip().is_loopback() && address.port() > 0);
    let root = std::fs::read(std::env::var_os("MISTRAL_WIRE_FIXTURE_CA").unwrap()).unwrap();
    let mut client = UpstreamClient::new(8);
    client.http = rquest::Client::builder()
        .tls_cert_store(rquest::tls::trust::CertStore::from_pem_stack(&root).unwrap())
        .no_proxy()
        .proxy(rquest::Proxy::all(format!("http://{address}")).unwrap())
        .redirect(rquest::redirect::Policy::none())
        .build()
        .unwrap();
    client
}

#[tokio::test]
#[ignore = "serves a bounded loopback Gateway with synthetic credentials for CLI comparison"]
async fn mistral_contract_public_wire_fixture() {
    let root = std::path::PathBuf::from(std::env::var_os("MISTRAL_WIRE_PUBLIC_FIXTURE").unwrap());
    let document = json!({
        "providers":[{
            "id":"mistral-wire", "preset":"mistral-vibe-api",
            "base_url":"https://api.mistral.ai/v1",
            "api_key":"mistral-fixture-secret",
            "supported_models":["mistral-large-latest","labs-leanstral-1-5"]
        }],
        "model_routes":[
            {"pattern":"mistral-large-latest","provider_ids":["mistral-wire"]},
            {"pattern":"labs-leanstral-1-5","provider_ids":["mistral-wire"]}
        ]
    });
    let mut fixture =
        console_support::ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), false);
    let state = std::sync::Arc::get_mut(&mut fixture.state).unwrap();
    state.local_runtime = Some(
        crate::local_runtime::LocalRuntime::open(&root)
            .await
            .unwrap(),
    );
    state.upstream_client = wire_fixture_client();
    // Public requests use the bounded-recovery client by default. Keep its
    // fixture trust and proxy local without altering production TLS settings.
    assert!(state
        .upstream_client
        .wait_http
        .set(state.upstream_client.http.clone())
        .is_ok());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    std::fs::write(root.join("gateway-address"), address.to_string()).unwrap();
    let server = crate::http::router::build_router(fixture.state.clone());
    let _ = tokio::time::timeout(Duration::from_secs(100), axum::serve(listener, server)).await;
}

#[test]
fn mistral_contract_vibe_ua_does_not_apply_to_nonofficial_base() {
    for base in [
        "https://api.mistral.ai.example.org/v1",
        "http://api.mistral.ai/v1",
        "https://api.mistral.ai@other.example.org/v1",
    ] {
        let payload = payload("mistral-vibe-api", base);
        assert!(
            !crate::upstream::headers::build_upstream_headers(&payload).contains_key("user-agent")
        );
    }
}

async fn receiver(
    streaming: bool,
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
                ("401 Unauthorized", "application/json", r#"{"message":"Unauthorized"}"#)
            } else if streaming {
                ("200 OK", "text/event-stream", concat!(
                    "data: {\"id\":\"fixture\",\"object\":\"chat.completion.chunk\",\"model\":\"mistral-large-latest\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\"},\"finish_reason\":null}]}\n\n",
                    "data: {\"id\":\"fixture\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                    "data: [DONE]\n\n"
                ))
            } else {
                ("200 OK", "application/json", r#"{"id":"fixture","object":"chat.completion","model":"mistral-large-latest","choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}"#)
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
async fn mistral_contract_wire_json_sse_and_auth_errors() {
    for (streaming, unauthorized) in [(false, false), (true, false), (false, true), (true, true)] {
        let (base, server) = receiver(streaming, unauthorized).await;
        let client = UpstreamClient::new(5);
        let payload = payload("mistral-studio-api", &base);
        let req = request(streaming);
        tokio::time::timeout(Duration::from_secs(7), async {
            if streaming {
                let result = client
                    .execute_stream(&payload, &req, "mistral-large-latest", None)
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
                    let body = response.text().await.unwrap();
                    assert!(body.contains("\"content\":\"OK\""));
                    assert!(body.ends_with("data: [DONE]\n\n"));
                }
            } else {
                let result = client
                    .execute(&payload, &req, "mistral-large-latest", None)
                    .await;
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
        assert!(headers.starts_with("post /v1/chat/completions http/1.1"));
        assert!(headers.contains("authorization: bearer mistral-fixture-secret"));
        assert!(!headers.contains("x-api-key:"));
        let body: Value = serde_json::from_slice(&captured[end + 4..]).unwrap();
        assert_eq!(body["model"], "mistral-large-latest");
        assert_eq!(body["stream"], streaming);
        assert_eq!(body["messages"][0]["content"], "Reply OK");
        assert!(!body.to_string().contains("mistral-fixture-secret"));
    }
}
