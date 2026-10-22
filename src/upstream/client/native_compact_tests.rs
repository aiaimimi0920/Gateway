//! Compact planning and real loopback transport, with synthetic credentials.
use super::*;
use bytes::Bytes;

fn payload(base: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"openai_compatible", "base_url":base,
        "api_key":"selected-secret", "request_client_profile":"codex_cli"}))
    .unwrap()
}

#[test]
fn compact_keeps_opaque_items_and_does_not_inject_generation_fields() {
    let body = json!({"model":"gpt-fixture","input":[
        {"type":"compaction","encrypted_content":"opaque"}],"future_field":true});
    let req = responses::normalize_responses(body.clone()).unwrap();
    for base in ["https://fixture.invalid", "https://fixture.invalid/v1"] {
        let plan = plan(&payload(base), &req, "gpt-fixture").unwrap();
        assert_eq!(plan.url, "https://fixture.invalid/v1/responses/compact");
        assert_eq!(plan.body.unwrap(), body);
    }
    let mut provider = payload("https://fixture.invalid");
    provider.responses_path = Some("/custom/responses?tenant=one".into());
    // An explicit Responses-only route opts into forced-stream conversion in
    // existing policy. This fixture advertises both native endpoint families.
    provider.chat_completions_path = Some("/v1/chat/completions".into());
    provider.extra_body = Some(
        serde_json::from_value(json!({"store":false,
        "instructions":"configured", "service_tier":"flex"}))
        .unwrap(),
    );
    let plan = plan(&provider, &req, "mapped").unwrap();
    assert_eq!(
        plan.url,
        "https://fixture.invalid/custom/responses/compact?tenant=one"
    );
    let body = plan.body.unwrap();
    assert_eq!(body["model"], "mapped");
    assert_eq!(body["instructions"], "configured");
    assert_eq!(body["service_tier"], "flex");
    assert!(body.get("store").is_none());
}

#[test]
fn compact_rejects_incompatible_transport_and_streaming() {
    let req = responses::normalize_responses(json!({"model":"gpt-fixture","input":[]})).unwrap();
    let mut provider = payload("https://fixture.invalid");
    provider.adapter = "anthropic_compatible".into();
    assert!(plan(&provider, &req, "gpt-fixture").is_err());
    let mut req = req;
    req.stream = true;
    assert!(plan(&payload("https://fixture.invalid"), &req, "gpt-fixture").is_err());
}

#[tokio::test]
async fn compact_native_transport_preserves_raw_request_and_replaces_credentials() {
    let (base, receiver) = super::super::tests::receiver().await;
    let raw = b"{ \"model\":\"gpt-fixture\", \"input\":[], \"instructions\":\"summarize\" }\n";
    let req = responses::normalize_responses(serde_json::from_slice(raw).unwrap()).unwrap();
    let wire = NativeRequestWire {
        native_session: None,
        compact: true,
        websocket: None,
        connection: None,
        body: Bytes::copy_from_slice(raw),
        query: Some("x=1&x=2".into()),
        client_request_id: false,
        header_order: vec![],
    };
    let extra = HashMap::from([
        ("authorization".into(), "Bearer caller-secret".into()),
        ("x-openai-subagent".into(), "compact".into()),
    ]);
    let reply = UpstreamClient::new(5)
        .execute_json_with_wire(
            "fixture",
            ProviderExecutionMode::DirectHttp,
            &payload(&base),
            &req,
            "gpt-fixture",
            Some(&extra),
            Some(&wire),
        )
        .await
        .unwrap();
    assert_eq!(reply["future_reply"], true);
    let captured = tokio::time::timeout(Duration::from_secs(5), receiver)
        .await
        .unwrap()
        .unwrap();
    let end = captured.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    assert_eq!(&captured[end + 4..], raw);
    let headers = std::str::from_utf8(&captured[..end]).unwrap();
    assert!(headers.starts_with("POST /v1/responses/compact?x=1&x=2 HTTP/1.1\r\n"));
    assert!(headers.contains("authorization: Bearer selected-secret\r\n"));
    assert!(headers.contains("x-openai-subagent: compact\r\n"));
    assert!(!headers.contains("caller-secret"));
}
