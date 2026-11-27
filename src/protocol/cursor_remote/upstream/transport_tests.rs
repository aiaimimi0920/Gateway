//! End-to-end loopback HTTP CONNECT proxy + Rustls ALPN `h2` + HTTP/2 duplex server tests.
use crate::protocol::cursor_remote::{
    runtime::worker::WorkerHooks,
    upstream::{
        self,
        protobuf::{connect_frame, encode_value, field_bytes, field_str, field_varint, fields},
        transport_live::LiveTransport,
        Input,
    },
};
use axum::http::Response;
use btls::{
    asn1::Asn1Time,
    bn::BigNum,
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    x509::{
        extension::{BasicConstraints, ExtendedKeyUsage, SubjectAlternativeName},
        X509NameBuilder, X509,
    },
};
use bytes::Bytes;
use gateway_cpa_http_transport::{async_compression::tokio::write::GzipEncoder, http2};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, OnceLock,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct CertFixture {
    ca_der: Vec<u8>,
    server_der: Vec<u8>,
    server_pkcs8: Vec<u8>,
}

fn cert_fixture() -> &'static CertFixture {
    static CERTS: OnceLock<CertFixture> = OnceLock::new();
    CERTS.get_or_init(|| {
        let ca_key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let ca = build_cert(1, "Cursor Loopback Test CA", &ca_key, None);
        let srv_key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let srv = build_cert(2, "api2.cursor.sh", &srv_key, Some((&ca, &ca_key)));
        CertFixture {
            ca_der: ca.to_der().unwrap(),
            server_der: srv.to_der().unwrap(),
            server_pkcs8: srv_key.private_key_to_der_pkcs8().unwrap(),
        }
    })
}

fn build_cert(
    serial: u32,
    cn: &str,
    key: &PKey<Private>,
    issuer: Option<(&X509, &PKey<Private>)>,
) -> X509 {
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", cn).unwrap();
    let name = name.build();
    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder
        .set_serial_number(&BigNum::from_u32(serial).unwrap().to_asn1_integer().unwrap())
        .unwrap();
    builder.set_subject_name(&name).unwrap();
    builder
        .set_issuer_name(issuer.map_or(name.as_ref(), |(ca, _)| ca.subject_name()))
        .unwrap();
    builder
        .set_not_before(&Asn1Time::from_unix(1_700_000_000).unwrap())
        .unwrap();
    builder
        .set_not_after(&Asn1Time::days_from_now(3650).unwrap())
        .unwrap();
    builder.set_pubkey(key).unwrap();
    let mut bc = BasicConstraints::new();
    if issuer.is_none() {
        bc.ca();
    }
    builder
        .append_extension(&bc.critical().build().unwrap())
        .unwrap();
    if issuer.is_some() {
        let ctx = builder.x509v3_context(issuer.map(|(ca, _)| &**ca), None);
        let san = SubjectAlternativeName::new()
            .dns("api2.cursor.sh")
            .dns("cursor.com")
            .dns("agentn.global.api5.cursor.sh")
            .build(&ctx)
            .unwrap();
        builder.append_extension(&san).unwrap();
        builder
            .append_extension(&ExtendedKeyUsage::new().server_auth().build().unwrap())
            .unwrap();
    }
    builder
        .sign(issuer.map_or(key, |(_, k)| k), MessageDigest::sha256())
        .unwrap();
    builder.build()
}

async fn gzip_bytes(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = GzipEncoder::new(&mut out);
    enc.write_all(raw).await.unwrap();
    enc.shutdown().await.unwrap();
    out
}

async fn spawn_connect_h2_server(reset_run: bool) -> (u16, tokio::task::JoinHandle<()>) {
    let certs = cert_fixture();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        certs.server_pkcs8.clone(),
    ));
    let mut srv_cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(
                certs.server_der.clone(),
            )],
            key,
        )
        .unwrap();
    srv_cfg.alpn_protocols = vec![b"h2".to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(srv_cfg));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let mut head = Vec::new();
                while !head.ends_with(b"\r\n\r\n") {
                    let Ok(b) = tcp.read_u8().await else { return };
                    head.push(b);
                }
                let head_str = String::from_utf8_lossy(&head);
                assert!(head_str.starts_with("CONNECT "));
                assert!(!head_str.to_ascii_lowercase().contains("authorization"));
                tcp.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .await
                    .unwrap();

                let Ok(tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let Ok(mut h2) = http2::server::handshake(tls).await else {
                    return;
                };
                while let Some(Ok((req, mut respond))) = h2.accept().await {
                    let path = req.uri().path().to_owned();
                    let mut body_stream = req.into_body();
                    tokio::spawn(async move {
                        if path == "/agent.v1.AgentService/Run" {
                            let first = body_stream.data().await.unwrap().unwrap();
                            body_stream
                                .flow_control()
                                .release_capacity(first.len())
                                .unwrap();
                            let resp = Response::builder()
                                .status(200)
                                .header("content-type", "application/connect+proto")
                                .body(())
                                .unwrap();
                            let mut send = respond.send_response(resp, false).unwrap();
                            if reset_run {
                                send.send_reset(http2::Reason::INTERNAL_ERROR);
                                return;
                            }
                            let mut has_tools = false;
                            if let Ok(top) = fields(&first[5..]) {
                                if let Some(run_f) = top.iter().find(|f| f.tag == 1) {
                                    if let Ok(rf) = fields(run_f.bytes) {
                                        has_tools =
                                            rf.iter().any(|f| f.tag == 4 && !f.bytes.is_empty());
                                    }
                                }
                            }
                            if has_tools {
                                let mut arg_pair = field_str(1, "cmd").unwrap();
                                arg_pair.extend_from_slice(
                                    &field_bytes(2, &encode_value(&json!("pwd"), 0).unwrap())
                                        .unwrap(),
                                );
                                let mut mcp = field_str(1, "mcp_gateway_exec").unwrap();
                                mcp.extend_from_slice(&field_bytes(2, &arg_pair).unwrap());
                                mcp.extend_from_slice(&field_str(3, "call_h2").unwrap());
                                mcp.extend_from_slice(&field_str(4, "gateway").unwrap());
                                mcp.extend_from_slice(&field_str(5, "exec").unwrap());
                                let mut exec_msg = field_varint(1, 11).unwrap();
                                exec_msg.extend_from_slice(&field_bytes(11, &mcp).unwrap());
                                send.send_data(
                                    Bytes::from(connect_frame(
                                        &field_bytes(2, &exec_msg).unwrap(),
                                        0,
                                    )),
                                    false,
                                )
                                .unwrap();
                                // Read client tool result continuation frame over live H2 stream!
                                let cont = body_stream.data().await.unwrap().unwrap();
                                assert_eq!(cont[0], 0);
                            }
                            let thinking = connect_frame(
                                &field_bytes(
                                    1,
                                    &field_bytes(4, &field_str(1, "h2-thinking").unwrap()).unwrap(),
                                )
                                .unwrap(),
                                0,
                            );
                            let answer = connect_frame(
                                &field_bytes(
                                    1,
                                    &field_bytes(1, &field_str(1, "CURSOR PURE RUST OK").unwrap())
                                        .unwrap(),
                                )
                                .unwrap(),
                                0,
                            );
                            let turn_end = connect_frame(
                                &field_bytes(1, &field_bytes(14, &[]).unwrap()).unwrap(),
                                0,
                            );
                            let end_stream = connect_frame(b"{}", 2);
                            let mut payload = thinking;
                            payload.extend_from_slice(&answer);
                            payload.extend_from_slice(&turn_end);
                            payload.extend_from_slice(&end_stream);
                            send.send_data(Bytes::from(payload), true).unwrap();
                            return;
                        }

                        // Drain unary request body
                        while let Some(Ok(chunk)) = body_stream.data().await {
                            let _ = body_stream.flow_control().release_capacity(chunk.len());
                        }
                        if path == "/aiserver.v1.AiService/AvailableModels" {
                            let mut variant = field_varint(3, 0).unwrap();
                            let _ = &mut variant;
                            let mut model_proto = field_str(1, "default").unwrap();
                            model_proto.extend_from_slice(&field_varint(5, 1).unwrap());
                            model_proto.extend_from_slice(&field_varint(46, 0).unwrap());
                            model_proto.extend_from_slice(&field_bytes(30, &variant).unwrap());
                            let body = field_bytes(2, &model_proto).unwrap();
                            let resp = Response::builder()
                                .status(200)
                                .header("content-type", "application/proto")
                                .body(())
                                .unwrap();
                            let mut send = respond.send_response(resp, false).unwrap();
                            send.send_data(Bytes::from(body), true).unwrap();
                            return;
                        }

                        let reply_json = match path.as_str() {
                            "/aiserver.v1.DashboardService/GetMe" => {
                                json!({"authId":"github|user123","email":"owner@example.test"})
                            }
                            "/agent.v1.AgentService/GetUsableModels"
                            | "/aiserver.v1.AiService/GetUsableModels" => {
                                json!({"models":[{"modelId":"default"}]})
                            }
                            "/agent.v1.AgentService/GetDefaultModelForCli"
                            | "/aiserver.v1.AiService/GetDefaultModelForCli" => {
                                json!({"model":{"modelId":"default"}})
                            }
                            "/aiserver.v1.ServerConfigService/GetServerConfig" => json!({
                                "http2Config":"FORCE_BIDI_ENABLED",
                                "agentUrlConfig":{
                                    "agentUrl":"https://agentn.global.api5.cursor.sh",
                                    "agentnUrl":"https://agentn.global.api5.cursor.sh",
                                    "localLoopInferenceUrl":"https://agentn.global.api5.cursor.sh"
                                },
                                "clientVersionStatus":{"updateLevel":"REQUIRED","minAllowedClientVersion":"3.18.0"}
                            }),
                            "/api/usage-summary" => json!({
                                "membershipType":"free",
                                "limitType":"user",
                                "individualUsage":{
                                    "onDemand":{"enabled":false,"used":0},
                                    "plan":{"totalPercentUsed":1.5}
                                }
                            }),
                            "/aiserver.v1.DashboardService/GetCreditGrantsBalance" => json!({}),
                            "/api/auth/stripe" => {
                                json!({"membershipType":"free","customerBalance":0})
                            }
                            "/aiserver.v1.DashboardService/GetTeams" => json!({}),
                            "/aiserver.v1.DashboardService/GetHardLimit" => {
                                json!({"noUsageBasedAllowed":true})
                            }
                            other => panic!("unexpected path: {other}"),
                        };
                        let gz = gzip_bytes(reply_json.to_string().as_bytes()).await;
                        let resp = Response::builder()
                            .status(200)
                            .header("content-type", "application/json")
                            .header("content-encoding", "gzip")
                            .body(())
                            .unwrap();
                        let mut send = respond.send_response(resp, false).unwrap();
                        send.send_data(Bytes::from(gz), true).unwrap();
                    });
                }
            });
        }
    });
    (port, handle)
}

fn sample_input(port: u16, with_tools: bool) -> Input {
    let exp = (time::OffsetDateTime::now_utc() + time::Duration::hours(1)).unix_timestamp();
    let token = crate::protocol::cursor_auth::tests::token("github|user123", exp);
    let email_hash = format!("{:x}", Sha256::digest(b"owner@example.test"));
    let mut raw = json!({
        "token": token,
        "emailHash": email_hash,
        "prompt": "2 + 3",
    });
    if with_tools {
        raw["tools"] = json!([{
            "name": "exec",
            "description": "run cmd",
            "parameters": {"type": "object"}
        }]);
    }
    Input::parse(&raw, port).unwrap()
}

#[tokio::test]
async fn live_transport_full_qualification_gzip_and_duplex_text_and_tools_succeed() {
    let (port, server) = spawn_connect_h2_server(false).await;
    let transport = LiveTransport::with_test_root(rustls::pki_types::CertificateDer::from(
        cert_fixture().ca_der.clone(),
    ));

    // 1. End-to-end pure-Rust text + thinking turn over CONNECT + TLS h2 + gzip unary preflight
    let sent_count = Arc::new(AtomicUsize::new(0));
    let sc = sent_count.clone();
    let uncertain = Arc::new(AtomicBool::new(false));
    let mut hooks = WorkerHooks::new(
        Box::new(|| Box::pin(async { Ok(()) })),
        Box::new(move || {
            sc.fetch_add(1, Ordering::SeqCst);
        }),
        uncertain.clone(),
        None,
    );
    let out = upstream::execute(sample_input(port, false), &mut hooks, &transport)
        .await
        .map_err(upstream::RunError::into_gateway)
        .unwrap();
    assert_eq!(out.text, "CURSOR PURE RUST OK");
    assert_eq!(out.thinking.as_deref(), Some("h2-thinking"));
    assert_eq!(out.requested_model, "default");
    assert_eq!(sent_count.load(Ordering::SeqCst), 1);
    assert!(!uncertain.load(Ordering::SeqCst));

    // 2. End-to-end pure-Rust duplex tool call + continuation over the SAME HTTP/2 stream
    let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(1);
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1);
    let tool_sent = Arc::new(AtomicUsize::new(0));
    let ts1 = tool_sent.clone();
    let ts2 = tool_sent.clone();
    let mut tool_hooks = WorkerHooks::new(
        Box::new(|| Box::pin(async { Ok(()) })),
        Box::new(move || {
            ts1.fetch_add(1, Ordering::SeqCst);
        }),
        Arc::new(AtomicBool::new(false)),
        Some(crate::protocol::cursor_remote::runtime::tools::Bridge {
            events: events_tx,
            commands: cmd_rx,
        }),
    );
    let driver = tokio::spawn(async move {
        let res = upstream::execute(sample_input(port, true), &mut tool_hooks, &transport)
            .await
            .map_err(upstream::RunError::into_gateway)
            .unwrap();
        res
    });
    let paused = events_rx.recv().await.unwrap().unwrap();
    let call = paused.call.unwrap();
    assert_eq!(call.id, "call_h2");
    assert_eq!(call.name, "exec");
    assert_eq!(call.arguments, r#"{"cmd":"pwd"}"#);
    cmd_tx
        .send(crate::protocol::cursor_remote::runtime::tools::Round {
            call_id: "call_h2".into(),
            output: "/workspace".into(),
            authorize: Box::new(|| Box::pin(async { Ok(()) })),
            sent: Box::new(move || {
                ts2.fetch_add(1, Ordering::SeqCst);
            }),
            uncertain: Arc::new(AtomicBool::new(false)),
        })
        .await
        .unwrap();
    let final_out = driver.await.unwrap();
    assert_eq!(final_out.text, "CURSOR PURE RUST OK");
    assert_eq!(tool_sent.load(Ordering::SeqCst), 2);

    server.abort();
}

#[tokio::test]
async fn live_transport_h2_stream_reset_fails_closed_with_http2_reset() {
    let (port, server) = spawn_connect_h2_server(true).await;
    let transport = LiveTransport::with_test_root(rustls::pki_types::CertificateDer::from(
        cert_fixture().ca_der.clone(),
    ));
    let mut hooks = WorkerHooks::new(
        Box::new(|| Box::pin(async { Ok(()) })),
        Box::new(|| {}),
        Arc::new(AtomicBool::new(false)),
        None,
    );
    let err = upstream::execute(sample_input(port, false), &mut hooks, &transport)
        .await
        .err()
        .unwrap()
        .into_gateway();
    assert_eq!(err.code.as_deref(), Some("cursor_remote_http2_reset"));
    server.abort();
}
