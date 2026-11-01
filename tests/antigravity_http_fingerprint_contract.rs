//! Opt-in local HTTPS A/B harness. Only synthetic credentials; no upstream forwarding.
#![cfg(feature = "cpa-rust-transport")]
use http_body_util::BodyExt;
use neuro_gateway::{routing::candidate::ProviderAccountPayload, upstream::antigravity};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

async fn head<S: tokio::io::AsyncRead + Unpin>(socket: &mut S) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        assert!(bytes.len() < 16384);
        bytes.push(socket.read_u8().await.unwrap());
    }
    String::from_utf8(bytes).unwrap()
}

async fn capture(
    listener: TcpListener,
    config: rustls::ServerConfig,
    accepted: bool,
) -> Vec<Value> {
    let (mut socket, address) = listener.accept().await.unwrap();
    assert!(address.ip().is_loopback());
    assert!(head(&mut socket)
        .await
        .starts_with("CONNECT daily-cloudcode-pa.googleapis.com:443 "));
    socket
        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
        .await
        .unwrap();
    let result = tokio_rustls::TlsAcceptor::from(Arc::new(config))
        .accept(socket)
        .await;
    if !accepted {
        assert!(
            result.is_err(),
            "untrusted or wrong-host certificate must be rejected"
        );
        return vec![];
    }
    let mut socket = result.unwrap();
    assert_eq!(socket.get_ref().1.alpn_protocol(), None);
    let mut captures = Vec::new();
    for index in 0..4 {
        let headers = head(&mut socket).await;
        eprintln!("local native-wire request {index} received");
        let lines: Vec<_> = headers.split("\r\n").filter(|s| !s.is_empty()).collect();
        let chunked = index >= 2;
        assert_eq!(lines.len(), 7, "no extra browser, Accept or proxy headers");
        let names: Vec<_> = lines[1..]
            .iter()
            .map(|s| s.split_once(':').unwrap().0)
            .collect();
        assert_eq!(
            names,
            [
                "Host",
                "User-Agent",
                if chunked {
                    "Transfer-Encoding"
                } else {
                    "Content-Length"
                },
                "Authorization",
                "Content-Type",
                "Accept-Encoding"
            ]
        );
        assert_eq!(lines[1], "Host: daily-cloudcode-pa.googleapis.com");
        assert_eq!(lines[2], "User-Agent: antigravity/hub/2.15.0 (aidev_client; os_type=windows; arch=amd64; cl=983516863)");
        assert_eq!(lines[4], "Authorization: Bearer synthetic-only");
        assert_eq!(lines[5], "Content-Type: application/json");
        assert_eq!(lines[6], "Accept-Encoding: gzip");
        let mut bytes = Vec::new();
        let mut chunks = Vec::new();
        if chunked {
            assert_eq!(lines[3], "Transfer-Encoding: chunked");
            loop {
                let mut size = String::new();
                loop {
                    let byte = socket.read_u8().await.unwrap();
                    size.push(char::from(byte));
                    assert!(size.len() < 32);
                    if size.ends_with("\r\n") {
                        break;
                    }
                }
                let size = usize::from_str_radix(size.trim(), 16).unwrap();
                assert!(bytes.len() + size <= 1024 * 1024);
                if size == 0 {
                    assert_eq!(socket.read_u16().await.unwrap(), 0x0d0a);
                    break;
                }
                chunks.push(size);
                let start = bytes.len();
                bytes.resize(start + size, 0);
                socket.read_exact(&mut bytes[start..]).await.unwrap();
                assert_eq!(socket.read_u16().await.unwrap(), 0x0d0a);
            }
            assert_eq!(chunks.len(), 1, "native emits one buffered JSON chunk");
        } else {
            let size: usize = lines[3]
                .strip_prefix("Content-Length: ")
                .unwrap()
                .parse()
                .unwrap();
            assert!(size <= 1024 * 1024);
            bytes.resize(size, 0);
            socket.read_exact(&mut bytes).await.unwrap();
        }
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        let expected_method = [
            "loadCodeAssist",
            "fetchAvailableModels",
            "streamGenerateContent?alt=sse",
            "streamGenerateContent?alt=sse",
        ][index];
        assert_eq!(
            lines[0],
            format!("POST /v1internal:{expected_method} HTTP/1.1")
        );
        match index {
            0 => assert_eq!(value, json!({"metadata":{"ideType":"ANTIGRAVITY"}})),
            1 => assert_eq!(value, json!({"project":"fixture-project"})),
            _ => {
                assert_eq!(value["requestType"], "agent");
                assert_eq!(value["userAgent"], "antigravity");
                assert_eq!(value["project"], "fixture-project");
                assert_eq!(value["model"], "fixture-model-343");
                assert!(value["requestId"].as_str().unwrap().starts_with("agent/"));
                assert!(value["requestId"].as_str().unwrap().ends_with("/1"));
            }
        }
        let reply = match index {
            0 => json!({"cloudaicompanionProject":"fixture-project"}).to_string(),
            1 => json!({"models":{"fixture-model-343":{}}}).to_string(),
            _ => format!(
                "data: {}\n\n",
                json!({"response":{"candidates":[{"content":{"role":"model","parts":[{"text":"OK"}]},"finishReason":"STOP"}]}})
            ),
        };
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\n\r\n{}",
                    reply.len(),
                    if chunked {
                        "text/event-stream"
                    } else {
                        "application/json"
                    },
                    reply
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let safe_headers: Vec<_> = lines[1..]
            .iter()
            .map(|s| {
                if s.starts_with("Authorization:") {
                    "Authorization: [redacted]".to_owned()
                } else {
                    (*s).to_owned()
                }
            })
            .collect();
        captures.push(
            json!({"request_line":lines[0], "headers":safe_headers, "chunk_sizes":chunks,
            "body":value}),
        );
    }
    captures
}

#[test]
#[ignore = "requires explicit isolated DER/PEM fixtures; no Google network traffic"]
fn antigravity_https_wire_and_certificate_verification() {
    // Full ingress has the same explicit stack budget as other Windows router contracts.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn run() {
    let root =
        PathBuf::from(std::env::var_os("ANTIGRAVITY_WIRE_FIXTURES").expect("fixture directory"));
    let temporary = root.join("runtime-temp");
    std::fs::create_dir_all(&temporary).unwrap();
    std::env::set_var("TEMP", &temporary);
    std::env::set_var("TMP", &temporary);
    let mode = std::env::var("ANTIGRAVITY_WIRE_MODE").unwrap_or_else(|_| "trusted".into());
    let accepted = mode == "trusted";
    let cert_name = if mode == "hostname" {
        "wrong-host.der"
    } else {
        "server.der"
    };
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![std::fs::read(root.join(cert_name)).unwrap().into()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(std::fs::read(root.join("key.der")).unwrap())
            .into(),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    std::env::set_var(
        "HTTPS_PROXY",
        format!("http://{}", listener.local_addr().unwrap()),
    );
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    if mode == "untrusted" {
        std::env::remove_var("GATEWAY_ANTIGRAVITY_CA_CERTIFICATE");
    } else {
        std::env::set_var("GATEWAY_ANTIGRAVITY_CA_CERTIFICATE", root.join("ca.pem"));
    }
    let client = rquest::Client::builder()
        .no_proxy()
        .default_headers(
            [
                (rquest::header::ACCEPT, "must-not-leak".parse().unwrap()),
                (rquest::header::COOKIE, "must-not-leak".parse().unwrap()),
            ]
            .into_iter()
            .collect(),
        )
        .build()
        .unwrap();
    let payload: ProviderAccountPayload = serde_json::from_value(json!({"adapter":"antigravity_oauth_compatible",
        "base_url":"https://daily-cloudcode-pa.googleapis.com/v1internal", "api_key":"synthetic-only",
        "extra_body":{"antigravity_project_id":"fixture-project"}})).unwrap();
    // Full ingress includes the fixture's bounded unavailable-Redis fallback waits.
    let (result, captures) = tokio::time::timeout(Duration::from_secs(45), async {
        tokio::join!(async {
            antigravity::probe(&client, &payload).await?;
            eprintln!("local probe complete; initializing public ingress fixture");
            let document = serde_json::from_value(json!({"providers":[{"id":"antigravity-pool","preset":"antigravity-oauth",
                "base_url":"https://daily-cloudcode-pa.googleapis.com/v1internal", "explicit_credentials_only":true,
                "supported_models":["fixture-model-343"], "extra_body":{"antigravity_project_id":"fixture-project"},
                "call_flow":{"special_adapter":"antigravity_oauth","fingerprint_adapter":"antigravity"},
                "credentials":[{"id":"synthetic","api_key":"synthetic-only","supported_models":["fixture-model-343"]}]}],
                "model_routes":[{"pattern":"fixture-model-343","provider_ids":["antigravity-pool"]}]})).unwrap();
            let fixture = support::ConsoleStateFixture::new(document, true);
            eprintln!("public ingress fixture initialized");
            for stream in [false, true] {
                let request = axum::http::Request::post("/v1/chat/completions")
                    .header("authorization", "Bearer public-api-contract-secret").header("content-type", "application/json")
                    .body(axum::body::Body::from(json!({"model":"fixture-model-343","stream":stream,
                        "messages":[{"role":"user","content":format!("Reply with exactly OK. Synthetic downstream stream={stream}.")}]}).to_string())).unwrap();
                let reply = Box::pin(neuro_gateway::http::router::build_router(fixture.state.clone()).oneshot(request)).await.unwrap();
                let status = reply.status();
                eprintln!("public ingress stream={stream} status={status}");
                let bytes = reply.into_body().collect().await.unwrap().to_bytes();
                assert_eq!(status, axum::http::StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
                if stream { assert!(String::from_utf8_lossy(&bytes).contains("OK")); }
                else { assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap()["choices"][0]["message"]["content"], "OK"); }
            }
            Ok::<_, neuro_gateway::error::GatewayError>(())
        }, capture(listener, config, accepted))
    }).await.expect("bounded local HTTPS run");
    assert_eq!(result.is_ok(), accepted, "{result:?}");
    assert_eq!(captures.len(), if accepted { 4 } else { 0 });
    std::fs::write(
        root.join(format!("gateway-http-{mode}.json")),
        serde_json::to_vec_pretty(&json!({
        "mode":mode,"accepted":accepted,"external_forwarding":false,"requests":captures}))
        .unwrap(),
    )
    .unwrap();
}
