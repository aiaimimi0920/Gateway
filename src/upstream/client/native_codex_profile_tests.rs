//! Non-Codex SDK input must not select the outbound Codex software envelope.
use super::*;
use bytes::Bytes;

fn request(native: bool, override_accept: bool, stream: bool) -> reqwest::Request {
    let mut payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "base_url":"https://chatgpt.com/backend-api/codex",
        "responses_path":"/responses", "request_client_profile":"codex_cli", "api_key":"selected",
        "headers":{"Chatgpt-Account-Id":"selected-account"}
    }))
    .unwrap();
    if override_accept {
        payload
            .headers
            .insert("Accept".into(), "configured/accept".into());
    }
    let raw = json!({"model":"gpt-fixture","input":"hello","stream":stream});
    let req = responses::normalize_responses(raw.clone()).unwrap();
    let mut extra = HashMap::from([
        ("accept".into(), "application/json".into()),
        ("user-agent".into(), "deepseek-harness/fixture".into()),
        ("x-client-request-id".into(), "caller-session".into()),
    ]);
    if native {
        extra.insert("originator".into(), "codex_exec".into());
        extra.insert("user-agent".into(), "codex_exec/0.155.1 fixture".into());
    }
    let wire = NativeRequestWire {
        native_session: None,
        compact: false,
        websocket: None,
        connection: None,
        body: Bytes::from(serde_json::to_vec(&raw).unwrap()),
        query: None,
        client_request_id: false,
        header_order: [
            "accept",
            "authorization",
            "x-client-request-id",
            "user-agent",
            "content-type",
            "originator",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    };
    let plan = UpstreamClient::build_request_plan(&payload, &req, "gpt-fixture", stream).unwrap();
    UpstreamClient::new(5)
        .native_request(
            &payload,
            &req,
            "gpt-fixture",
            Some(&extra),
            Some(&wire),
            &plan,
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn native_profile_normalizes_sdk_stream_accept() {
    let request = request(false, false, true);
    assert_eq!(request.headers()["accept"], "text/event-stream");
    assert_eq!(request.headers()["authorization"], "Bearer selected");
    assert_eq!(request.headers()["x-client-request-id"], "caller-session");
}

#[test]
fn native_profile_uses_codex_order_for_sdk_identity() {
    let request = request(false, false, true);
    let names: Vec<_> = request.headers().keys().map(|name| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "x-client-request-id",
            "accept",
            "content-type",
            "authorization",
            "chatgpt-account-id",
            "originator",
            "user-agent"
        ]
    );
}

#[test]
fn native_profile_preserves_native_and_operator_accept() {
    assert_eq!(
        request(true, false, true).headers()["accept"],
        "application/json"
    );
    assert_eq!(
        request(false, true, true).headers()["accept"],
        "configured/accept"
    );
    assert_eq!(
        request(false, false, false).headers()["accept"],
        "application/json"
    );
    let native = request(true, false, true);
    assert_eq!(native.headers().keys().next().unwrap(), "accept");
}

#[test]
fn native_profile_sdk_envelope_is_explicit_and_does_not_create_session_state() {
    let request = request(false, false, true);
    for absent in [
        "cookie",
        "session-id",
        "thread-id",
        "x-codex-turn-metadata",
        "x-codex-beta-features",
    ] {
        assert!(!request.headers().contains_key(absent));
    }
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "base_url":"https://example.invalid", "api_key":"fixture"
    }))
    .unwrap();
    let req =
        responses::normalize_responses(json!({"model":"fixture","input":"hello","stream":true}))
            .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("accept", HeaderValue::from_static("application/json"));
    let extra = HashMap::from([("user-agent".into(), "sdk/1".into())]);
    let wire = NativeRequestWire {
        native_session: None,
        compact: false,
        websocket: None,
        connection: None,
        body: Bytes::new(),
        query: None,
        client_request_id: false,
        header_order: vec!["accept".into()],
    };
    assert!(profile::sdk_envelope(&mut headers, &payload, &req, Some(&extra), &wire).is_none());
    assert_eq!(headers["accept"], "application/json");
}
