//! HTTP success is insufficient: only completed assistant text may restore exact model health.
use super::*;

#[test]
fn qwen_probe_requires_complete_bounded_text_and_redacts_every_report() {
    run(responses);
}
async fn responses() {
    let server = wire::Server::start().await;
    let mut fixture = fixture();
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = server.client();
    let grant = grant_console_secret_access(&fixture.state).await;
    let cases = [
        ("OK", "passed", "Reply: OK"),
        ("echo", "passed", "[redacted]"),
        ("401", "failed", "HTTP 401"),
        ("403", "failed", "HTTP 403"),
        ("429", "failed", "HTTP 429"),
        ("redirect", "failed", "HTTP 307"),
        ("invalid-json", "failed", "invalid JSON"),
        ("invalid-utf8", "failed", "invalid JSON"),
        ("interrupted", "failed", "Model call failed"),
        ("empty", "failed", "completed"),
        ("missing-stop", "failed", "completed"),
        ("length", "failed", "completed"),
        ("filtered", "failed", "completed"),
        ("wrong-role", "failed", "completed"),
        ("tools", "failed", "completed"),
        ("refusal", "failed", "completed"),
        ("error-envelope", "failed", "completed"),
        ("oversize", "failed", "64 KiB"),
        ("slow", "failed", "Model call failed"),
    ];
    for (prompt, expected, message) in cases {
        let response = with_transport(
            &server.origin,
            send_console_json(
                &fixture.state,
                Method::POST,
                ENDPOINT,
                json!({"prompt":prompt,"model":"probe-alias","credentialIds":["exact"]}),
                Some(&grant),
                None,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["status"], expected, "{prompt}: {body}");
        assert!(
            result["message"].as_str().unwrap().contains(message),
            "{prompt}: {body}"
        );
        assert_eq!(result["providerId"], "qwen-probe");
        assert_eq!(result["credentialId"], "exact");
        assert!(!body.to_string().contains("synthetic-"), "{prompt}: {body}");
    }
    assert_eq!(server.calls().len(), cases.len());
    // No hidden retry and no redirect replay, including the 307 response above.
    assert_eq!(server.attempts(), cases.len());
    server.stop().await;
}

#[test]
fn qwen_probe_product_and_flow_mutations_fail_before_transport() {
    run(invalid_product);
}
async fn invalid_product() {
    let server = wire::Server::start().await;
    let mut fixture = fixture();
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = server.client();
    let request =
        super::super::ConsoleProbeRequest::parse(br#"{"prompt":"OK","model":"probe-alias"}"#)
            .unwrap()
            .unwrap();
    for mutation in 0..6 {
        let mut target = fixture
            .state
            .route_config
            .snapshot()
            .select_credential_probe_target("exact")
            .unwrap();
        match mutation {
            0 => target.payload.base_url = "https://example.invalid/v1".into(),
            1 => target.payload.auth_mode = Some("oauth".into()),
            2 => target.payload.responses_path = Some("/responses".into()),
            3 => target.payload.headers.insert("Cookie".into(), "synthetic-cookie".into()).map(|_| ()).unwrap_or(()),
            4 => target.payload.call_flow = Some(serde_json::from_value(json!({"call_adapter":"anthropic", "fingerprint_adapter":"auto", "special_adapter":"none"})).unwrap()),
            _ => target.payload.api_key = "synthetic key with space".into(),
        }
        let (_, report) = with_transport(
            &server.origin,
            super::super::probe_console_target_with_request(
                &fixture.state,
                &target,
                Some(&request),
            ),
        )
        .await;
        assert_ne!(
            report.status,
            super::super::ProviderPayloadProbeStatus::Passed
        );
        assert!(report.answer.is_none());
        assert!(!report.message.contains("synthetic-exact-key"));
        assert!(!report.message.contains("synthetic-cookie"));
        assert!(!report.message.contains("synthetic key with space"));
    }
    assert_eq!(server.attempts(), 0);
    server.stop().await;
}

#[test]
fn qwen_probe_dropped_management_future_closes_pending_http_without_success() {
    run(cancelled);
}
async fn cancelled() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (ready, received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        loop {
            let mut chunk = [0; 4096];
            let count = socket.read(&mut chunk).await.unwrap();
            assert!(count > 0 && request.len() + count <= 32768);
            request.extend_from_slice(&chunk[..count]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let size: usize = header
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if request.len() == end + 4 + size {
                    break;
                }
            }
        }
        assert!(String::from_utf8_lossy(&request).contains("synthetic-exact-key"));
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n{").await.unwrap();
        ready.send(()).unwrap();
        let mut byte = [0; 1];
        let closed = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut byte))
            .await
            .unwrap();
        assert!(matches!(closed, Ok(0)) || closed.is_err());
    });
    let mut fixture = fixture();
    let directory =
        std::env::temp_dir().join(format!("qwen-cancel-probe-{}", uuid::Uuid::new_v4()));
    let local = crate::local_runtime::LocalRuntime::open(&directory)
        .await
        .unwrap();
    {
        let state = Arc::get_mut(&mut fixture.state).unwrap();
        state.upstream_client.http = rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&origin).unwrap())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        state.local_runtime = Some(local.clone());
    }
    let grant = grant_console_secret_access(&fixture.state).await;
    let mut operation = Box::pin(with_transport(
        &origin,
        send_console_json(
            &fixture.state,
            Method::POST,
            ENDPOINT,
            json!({"prompt":"OK","model":"probe-alias","credentialIds":["exact"]}),
            Some(&grant),
            None,
        ),
    ));
    tokio::select! {
        reply = &mut operation => panic!("Pending response unexpectedly completed: {}", reply.status()),
        ready = received => ready.unwrap(),
        _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("Local cancellation fixture did not receive the call"),
    }
    drop(operation);
    tokio::time::timeout(Duration::from_secs(4), server)
        .await
        .unwrap()
        .unwrap();
    let audits = local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 1);
    assert!(!audits.iter().any(|a| a.status == "completed"));
    // This does not assert cancellation finalization: the shared recording owner still lacks it.
    let states = local
        .list_model_states(crate::db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(!states.iter().any(|s| s.status == "active"));
    drop(fixture);
    local.close().await;
}
