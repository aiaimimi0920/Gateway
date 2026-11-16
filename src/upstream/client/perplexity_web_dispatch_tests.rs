//! Exercise real client shortcuts; no configurable production URL or historical credentials.
use super::*;
use crate::protocol::perplexity_web::material as product;
use crate::upstream::perplexity_web::test_support as wire;
#[path = "perplexity_web_snapshot_tests.rs"]
mod snapshots;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(serde_json::json!({
        "adapter":product::ADAPTER,"auth_mode":product::AUTH_MODE,"base_url":product::BASE_URL,
        "api_key":"synthetic-session","headers":{"User-Agent":"synthetic-agent"},
        "session_auth":{"transport":"cookie","primaryCookieName":product::SESSION_COOKIE},
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"perplexity_web"}
    })).unwrap()
}
fn request() -> CanonicalRelayRequest {
    crate::protocol::responses::normalize_responses(serde_json::json!({
        "model":"public-alias","input":"合成问题"
    }))
    .unwrap()
}
fn client(server: &wire::Server) -> UpstreamClient {
    let mut client = UpstreamClient::new(300);
    client.http = server.client();
    client
}
#[tokio::test]
async fn perplexity_web_dispatch_uses_resolved_model_and_ignores_caller_auth_headers() {
    let server = wire::Server::start(wire::completed(), 200, true).await;
    let client = client(&server);
    let headers = std::collections::HashMap::from([
        ("authorization".into(), "Bearer synthetic-caller-key".into()),
        ("cookie".into(), "foreign=synthetic-caller-cookie".into()),
        ("user-agent".into(), "foreign-agent".into()),
    ]);
    let answer = wire::within(
        &server.origin,
        client.execute_with_provider_account_id(
            "exact",
            &payload(),
            &request(),
            "turbo",
            Some(&headers),
        ),
    )
    .await
    .expect("Dedicated Web dispatch must accept a caller alias resolved to turbo");
    assert_eq!(answer.text, "合成答案");
    assert_eq!(answer.sources.len(), 1);
    assert!(answer.usage.is_none() && answer.model_declaration.is_none());
    let calls = server.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].path, product::ASK_PATH);
    assert_eq!(
        calls[0].headers["cookie"],
        "__Secure-next-auth.session-token=synthetic-session"
    );
    assert_eq!(calls[0].headers["user-agent"], "synthetic-agent");
    assert!(!calls[0].headers.contains_key("authorization"));
    assert_eq!(calls[0].body["params"]["model_preference"], "turbo");
}
#[tokio::test]
async fn perplexity_web_dispatch_aggregated_stream_keeps_sources_without_usage() {
    let server = wire::Server::start(wire::completed(), 200, false).await;
    let client = client(&server);
    let response = wire::within(
        &server.origin,
        client.execute_stream(&payload(), &request(), "turbo", None),
    )
    .await
    .expect("Dedicated Web stream dispatch must retain the native live owner");
    let UpstreamStreamingResponse::NativeEvents(mut stream) = response else {
        panic!("Expected live snapshot observations and late canonical bytes")
    };
    let mut output = Vec::new();
    while let Some(bytes) = stream.next().await {
        output.extend_from_slice(&bytes.unwrap());
    }
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("合成答案") && output.contains("gateway_sources"));
    assert!(!output.contains("synthetic-private-followup") && !output.contains("\"usage\":{"));
    assert_eq!(server.calls().len(), 1);
}

#[tokio::test]
async fn perplexity_web_dispatch_refuses_unsupported_input_and_all_raw_shortcuts_before_network() {
    let server = wire::Server::start(wire::completed(), 200, false).await;
    let client = client(&server);
    let payload = payload();
    for (endpoint, model) in [
        (EndpointKind::Messages, "turbo"),
        (EndpointKind::ChatCompletions, "sonar"),
        (EndpointKind::Embeddings, "turbo"),
    ] {
        let mut request = request();
        request.endpoint_kind = endpoint;
        let error = client
            .execute(&payload, &request, model, None)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("perplexity_web_unsupported_request")
        );
    }
    let error = client
        .execute_json_passthrough(
            "exact",
            ProviderExecutionMode::DirectHttp,
            &payload,
            &request(),
            "turbo",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("perplexity_web_request_plan_unsupported")
    );
    let error = client
        .execute_binary_passthrough(&payload, &request(), "turbo", None)
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("perplexity_web_request_plan_unsupported")
    );
    assert!(!UpstreamClient::uses_native_codex(
        &payload,
        &request(),
        "turbo"
    ));
    assert!(!UpstreamClient::uses_native_claude(
        &payload,
        &request(),
        "turbo"
    ));
    let compact = crate::protocol::codex_wire::NativeRequestWire {
        native_session: None,
        compact: true,
        websocket: None,
        connection: None,
        body: bytes::Bytes::new(),
        query: None,
        client_request_id: false,
        header_order: Vec::new(),
    };
    assert!(client
        .execute_json_with_wire(
            "exact",
            ProviderExecutionMode::DirectHttp,
            &payload,
            &request(),
            "turbo",
            None,
            Some(&compact)
        )
        .await
        .is_err());
    assert_eq!(server.attempts(), 0);
}
