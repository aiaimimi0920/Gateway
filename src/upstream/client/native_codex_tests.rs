//! Wire-level regressions use a bounded loopback receiver, no real provider or account.
use super::*;
use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn native_codex_oauth_stream_does_not_enter_forced_stream_aggregation() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "base_url":"https://chatgpt.com/backend-api/codex",
        "responses_path":"/responses", "request_client_profile":"codex_cli", "api_key":"fixture"
    }))
    .unwrap();
    for stream in [false, true] {
        let req = responses::normalize_responses(json!({
            "model":"gpt-fixture", "input":"hello", "stream":stream
        }))
        .unwrap();
        assert!(payload.prefers_forced_streaming_responses(req.endpoint_kind));
        assert_eq!(
            UpstreamClient::uses_native_codex(&payload, &req, "gpt-fixture"),
            stream
        );
    }
}

#[test]
fn native_codex_oauth_plan_preserves_session_fields_and_input_roles() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "base_url":"https://chatgpt.com/backend-api/codex",
        "responses_path":"/responses", "request_client_profile":"codex_cli", "api_key":"fixture"
    }))
    .unwrap();
    let raw = json!({"model":"gpt-fixture", "stream":true, "store":false,
        "input":[{"role":"developer","type":"message","content":"preserve"}],
        "prompt_cache_key":"cache", "client_metadata":{"opaque":"state"}, "tools":[]});
    let req = responses::normalize_responses(raw.clone()).unwrap();
    let plan = UpstreamClient::build_request_plan(&payload, &req, "gpt-fixture", true).unwrap();
    assert_eq!(plan.body.unwrap(), raw);
}

pub(super) async fn receiver() -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        loop {
            let mut block = [0; 4096];
            let read = socket.read(&mut block).await.unwrap();
            assert!(read > 0 && request.len() + read <= 32_768);
            request.extend_from_slice(&block[..read]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..end]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::to_owned)
                    })
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if request.len() == end + 4 + length {
                    break;
                }
            }
        }
        let reply =
            b"{\"id\":\"fixture\",\"status\":\"completed\",\"output\":[],\"future_reply\":true}";
        let header = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", reply.len());
        socket.write_all(header.as_bytes()).await.unwrap();
        socket.write_all(reply).await.unwrap();
        request
    });
    (url, task)
}

#[tokio::test]
async fn native_transport_retains_bytes_query_identity_and_header_order() {
    for stream in [false, true] {
        let (base, server) = receiver().await;
        let payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"openai_compatible", "base_url":base, "api_key":"selected-secret",
            "request_client_profile":"codex_cli", "headers":{"Chatgpt-Account-Id":"selected-account"}
        }))
        .unwrap();
        let raw = format!("{{ \"input\": [{{\"role\":\"developer\",\"type\":\"message\",\"id\":\"native\",\"content\":\"unchanged\"}}], \"instructions\":\"native instructions\", \"tools\": [], \"model\":\"gpt-fixture\", \"stream\":{stream}, \"store\":false, \"include\":[\"reasoning.encrypted_content\"], \"parallel_tool_calls\":true, \"client_metadata\":{{\"opaque\":true}} }}\n");
        let req = responses::normalize_responses(serde_json::from_str(&raw).unwrap()).unwrap();
        let wire = NativeRequestWire {
            native_session: None,
            compact: false,
            websocket: None,
            connection: None,
            body: Bytes::from(raw.clone()),
            query: Some("repeat=1&repeat=2&encoded=%2F+a".into()),
            client_request_id: false,
            header_order: [
                "session-id",
                "accept",
                "content-type",
                "authorization",
                "originator",
                "user-agent",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        };
        let extra = HashMap::from([
            ("session-id".into(), "session-fixture".into()),
            ("originator".into(), "codex_exec".into()),
            (
                "user-agent".into(),
                "codex_exec/0.155.1 (Windows 10.0.26200; x86_64) unknown (codex_exec; 0.155.1)"
                    .into(),
            ),
            ("authorization".into(), "Bearer inbound-secret".into()),
            ("x-request-id".into(), "gateway-generated".into()),
        ]);
        let client = UpstreamClient::new(5);
        if stream {
            assert!(matches!(
                client
                    .execute_stream_with_wire(
                        &payload,
                        &req,
                        "gpt-fixture",
                        Some(&extra),
                        Some(&wire)
                    )
                    .await
                    .unwrap(),
                UpstreamStreamingResponse::Native(_)
            ));
        } else {
            let reply = client
                .execute_json_with_wire(
                    "fixture",
                    ProviderExecutionMode::DirectHttp,
                    &payload,
                    &req,
                    "gpt-fixture",
                    Some(&extra),
                    Some(&wire),
                )
                .await
                .unwrap();
            assert_eq!(reply["future_reply"], true);
        }
        let captured = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        let end = captured.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        assert_eq!(&captured[end + 4..], raw.as_bytes());
        let header = String::from_utf8(captured[..end].to_vec()).unwrap();
        assert!(
            header.starts_with("POST /v1/responses?repeat=1&repeat=2&encoded=%2F+a HTTP/1.1\r\n"),
            "{header}"
        );
        assert!(header.contains("authorization: Bearer selected-secret\r\n"));
        assert!(!header.contains("inbound-secret"));
        for forbidden in [
            "sec-ch-",
            "sec-fetch-",
            "accept-encoding",
            "x-request-id",
            "version:",
            "upgrade-insecure-requests",
            "x-openai-subagent",
            "x-openai-internal-codex-residency",
            "x-codex-parent-thread-id",
            "x-codex-routing-hint",
            "x-openai-memgen-request",
        ] {
            assert!(!header.contains(forbidden), "{header}");
        }
        let names: Vec<_> = header
            .lines()
            .skip(1)
            .map(|line| line.split(':').next().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "session-id",
                "accept",
                "content-type",
                "authorization",
                "chatgpt-account-id",
                "originator",
                "user-agent",
                "host",
                "content-length"
            ]
        );
    }
}

#[tokio::test]
async fn native_transport_preserves_optional_codex_metadata_without_forwarding_controls() {
    for (source, configured) in [
        ("review", false),
        ("compact", false),
        ("memory_consolidation", false),
        ("collab_spawn", false),
        ("custom-agent", false),
        ("review", true),
    ] {
        let (base, server) = receiver().await;
        let mut payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"openai_compatible", "base_url":base, "api_key":"selected-secret",
            "request_client_profile":"codex_cli"
        }))
        .unwrap();
        if configured {
            payload
                .headers
                .insert("X-OpenAI-Subagent".into(), "configured".into());
        }
        let raw =
            br#"{"model":"gpt-fixture","input":"hello","instructions":"fixture","stream":false}"#;
        let req = responses::normalize_responses(serde_json::from_slice(raw).unwrap()).unwrap();
        let wire = NativeRequestWire {
            native_session: None,
            compact: false,
            websocket: None,
            connection: None,
            body: Bytes::from_static(raw),
            query: None,
            client_request_id: false,
            header_order: vec![
                "x-openai-subagent".into(),
                "x-openai-internal-codex-residency".into(),
                "x-codex-parent-thread-id".into(),
                "x-codex-routing-hint".into(),
                "x-openai-memgen-request".into(),
            ],
        };
        let extra = HashMap::from([
            ("x-openai-subagent".into(), source.into()),
            ("x-openai-internal-codex-residency".into(), "us".into()),
            ("x-codex-parent-thread-id".into(), "fixture-parent".into()),
            ("x-codex-routing-hint".into(), "fixture-route".into()),
            ("x-openai-memgen-request".into(), "true".into()),
            (
                "x-openai-internal-codex-responses-lite".into(),
                "true".into(),
            ),
            ("cookie".into(), "private-cookie".into()),
            ("authorization".into(), "Bearer inbound-secret".into()),
            ("x-neuro-account-group".into(), "private-group".into()),
            ("x-internal-api-key".into(), "private-control".into()),
        ]);
        UpstreamClient::new(5)
            .execute_json_with_wire(
                "fixture",
                ProviderExecutionMode::DirectHttp,
                &payload,
                &req,
                "gpt-fixture",
                Some(&extra),
                Some(&wire),
            )
            .await
            .unwrap();
        let captured = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        let end = captured.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let headers = std::str::from_utf8(&captured[..end]).unwrap();
        let expected = if configured { "configured" } else { source };
        assert!(headers.starts_with(&format!(
            "POST /v1/responses HTTP/1.1\r\nx-openai-subagent: {expected}\r\nx-openai-internal-codex-residency: us\r\nx-codex-parent-thread-id: fixture-parent\r\nx-codex-routing-hint: fixture-route\r\nx-openai-memgen-request: true\r\n"
        )), "{headers}");
        assert!(headers.contains("authorization: Bearer selected-secret\r\n"));
        assert!(headers.contains("x-openai-internal-codex-responses-lite: true\r\n"));
        for forbidden in [
            "private-cookie",
            "inbound-secret",
            "private-group",
            "private-control",
        ] {
            assert!(!headers.contains(forbidden), "{headers}");
        }
    }
}
