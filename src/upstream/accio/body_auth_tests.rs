//! Exercise credential ownership through the production planner and its generic merge.
use crate::protocol::canonical::{EndpointKind, ProtocolFamily};
use crate::protocol::openai::normalize_chat_completions;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use serde_json::{json, Value};

fn payload(key: &str, extra: Value) -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter":"accio_compatible", "baseUrl":"https://phoenix-gw.alibaba.com",
        "apiKey":key, "headers":{"utdid":"synthetic-device"}, "extraBody":extra
    }))
    .unwrap()
}

fn plan(
    payload: &ProviderAccountPayload,
    caller: Value,
) -> Result<Value, crate::error::GatewayError> {
    let mut request = normalize_chat_completions(json!({
        "model":"caller-alias", "messages":[{"role":"user","content":"hello"}]
    }))
    .unwrap();
    request.extra = serde_json::from_value(caller).unwrap();
    Ok(
        UpstreamClient::build_request_plan(payload, &request, "resolved-model", false)?
            .body
            .unwrap(),
    )
}

fn assert_error(payload: &ProviderAccountPayload, caller: Value, code: &str) {
    let error = plan(payload, caller).expect_err("invalid credential must not produce a plan");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some(code));
    assert!(!error.retryable);
    assert!(!format!("{error:?}").contains("synthetic-secret"));
}

#[test]
fn current_access_wins_after_rotation_for_all_ingress_families_and_modes() {
    let mut payload = payload(
        "synthetic-current",
        json!({
            "token":"synthetic-old", "accessToken":"synthetic-old-alias", "temperature":0.25
        }),
    );
    payload.call_flow = Some(
        serde_json::from_value(json!({
            "call_adapter":"auto", "fingerprint_adapter":"session", "special_adapter":"accio"
        }))
        .unwrap(),
    );
    for (family, endpoint) in [
        (ProtocolFamily::OpenAi, EndpointKind::ChatCompletions),
        (ProtocolFamily::OpenAi, EndpointKind::Responses),
        (ProtocolFamily::Anthropic, EndpointKind::Messages),
        (
            ProtocolFamily::GeminiGenerateContent,
            EndpointKind::ChatCompletions,
        ),
    ] {
        for stream in [false, true] {
            let mut req = normalize_chat_completions(json!({
                "model":"caller-alias", "messages":[{"role":"user","content":"hello"}],
                "token":"synthetic-caller", "accessToken":"synthetic-caller-alias"
            }))
            .unwrap();
            req.protocol_family = family;
            req.endpoint_kind = endpoint;
            req.stream = stream;
            let before = payload.extra_body.clone();
            let body = UpstreamClient::build_request_plan(&payload, &req, "resolved-model", stream)
                .unwrap()
                .body
                .unwrap();
            assert_eq!(body["token"], "synthetic-current");
            assert!(body.get("accessToken").is_none());
            assert_eq!(body["incremental"], stream);
            assert_eq!(body["model"], "resolved-model");
            assert_eq!(body["temperature"], 0.25);
            assert_eq!(
                payload.extra_body, before,
                "planning must not mutate the stored credential"
            );
        }
    }
}

#[test]
fn legacy_token_only_configuration_remains_usable() {
    let payload = payload("", json!({"token":"synthetic-legacy"}));
    let body = plan(&payload, json!({"token":"synthetic-caller"})).unwrap();
    assert_eq!(body["token"], "synthetic-legacy");
}

#[test]
fn legacy_access_token_alias_normalizes_to_the_canonical_wire_field() {
    let body = plan(
        &payload("", json!({"accessToken":"synthetic-legacy"})),
        json!({}),
    )
    .unwrap();
    assert_eq!(body["token"], "synthetic-legacy");
    assert!(body.get("accessToken").is_none());
}

#[test]
fn equal_legacy_aliases_are_not_ambiguous() {
    let body = plan(
        &payload(
            "",
            json!({
                "token":"synthetic-legacy", "accessToken":"synthetic-legacy"
            }),
        ),
        json!({}),
    )
    .unwrap();
    assert_eq!(body["token"], "synthetic-legacy");
    assert!(body.get("accessToken").is_none());
}

#[test]
fn conflicting_legacy_aliases_are_rejected_without_choosing_an_identity() {
    assert_error(
        &payload(
            "",
            json!({
                "token":"synthetic-secret-one", "accessToken":"synthetic-secret-two"
            }),
        ),
        json!({}),
        "accio_ambiguous_body_token",
    );
}

#[test]
fn malformed_present_legacy_alias_is_rejected() {
    for value in [
        Value::Null,
        json!(123),
        json!({"secret":"synthetic-secret"}),
        json!(" "),
    ] {
        assert_error(
            &payload("", json!({"token":"synthetic-secret", "accessToken":value})),
            json!({}),
            "accio_invalid_body_token",
        );
    }
}

#[test]
fn missing_trusted_access_does_not_borrow_caller_credentials() {
    for key in ["", " \t"] {
        assert_error(
            &payload(key, json!({})),
            json!({
                "token":"synthetic-secret", "accessToken":"synthetic-secret"
            }),
            "accio_missing_body_token",
        );
    }
}

#[test]
fn current_access_ignores_obsolete_malformed_legacy_copies() {
    let body = plan(
        &payload("synthetic-current", json!({"token":null,"accessToken":42})),
        json!({}),
    )
    .unwrap();
    assert_eq!(body["token"], "synthetic-current");
    assert!(body.get("accessToken").is_none());
}

#[test]
fn trusted_device_overrides_caller_and_stale_extra_body() {
    let body = plan(
        &payload(
            "synthetic-current",
            json!({
                "utdid":"synthetic-old-device", "x-utdid":"synthetic-old-device"
            }),
        ),
        json!({"utdid":"synthetic-caller-device", "x-utdid":"synthetic-caller-device"}),
    )
    .unwrap();
    assert_eq!(body["utdid"], "synthetic-device");
    assert!(body.get("x-utdid").is_none());
}

#[test]
fn missing_or_invalid_trusted_device_cannot_be_supplied_by_caller() {
    for headers in [
        json!({}),
        json!({"utdid":" "}),
        json!({"utdid":"bad\nvalue"}),
    ] {
        let mut payload = payload(
            "synthetic-current",
            json!({"utdid":"synthetic-extra-device"}),
        );
        payload.headers = serde_json::from_value(headers).unwrap();
        assert_error(
            &payload,
            json!({"utdid":"synthetic-caller-device"}),
            "accio_invalid_utdid",
        );
    }
}

#[test]
fn legacy_x_utdid_header_remains_supported() {
    let mut payload = payload("synthetic-current", json!({}));
    payload.headers = serde_json::from_value(json!({"x-utdid":"synthetic-alias-device"})).unwrap();
    assert_eq!(
        plan(&payload, json!({})).unwrap()["utdid"],
        "synthetic-alias-device"
    );
}

#[test]
fn credential_aliases_and_refresh_material_are_not_forwarded_as_body_extras() {
    let mut extras = json!({"temperature":0.5, "conversation_id":"synthetic-conversation"});
    for key in [
        "apiKey",
        "api_key",
        "access_token",
        "refreshToken",
        "refresh_token",
        "sgcookie",
    ] {
        extras[key] = json!("synthetic-secret");
    }
    let body = plan(&payload("synthetic-current", extras.clone()), extras).unwrap();
    assert_eq!(body["token"], "synthetic-current");
    assert_eq!(body["temperature"], 0.5);
    assert_eq!(body["conversation_id"], "synthetic-conversation");
    assert!(!body.to_string().contains("synthetic-secret"));
}

#[test]
fn real_routing_runtime_override_reaches_body_without_rewriting_legacy_copies() {
    use crate::routing::config::{future_rfc3339_after_secs, RouteConfigStore};
    let store = RouteConfigStore::from_document(serde_json::from_value(json!({
        "providers":[{"id":"synthetic-accio", "preset":"accio", "base_url":"https://phoenix-gw.alibaba.com",
            "explicit_credentials_only":true, "credentials":[{
                "id":"synthetic-account", "api_key":"synthetic-old-access",
                "refresh_token":"synthetic-old-refresh", "expires_at":"2020-01-01T00:00:00Z",
                "refresh_endpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token",
                "refresh_client_id":"synthetic-client", "headers":{"utdid":"synthetic-device"},
                "extra_body":{"token":"synthetic-old-access", "accessToken":"synthetic-old-access"}
            }]}]
    })).unwrap()).unwrap();
    let snapshot = store.snapshot();
    let (_, credential) = snapshot
        .oauth_refresh_target("synthetic-accio", "synthetic-account")
        .unwrap();
    let state = credential.refresh_config.as_ref().unwrap();
    let attempt = state
        .begin_refresh(std::time::Duration::MAX, &credential.payload.api_key)
        .unwrap();
    // Apply synthetic material directly: no transport, durable grant, or refresh readiness is claimed.
    attempt
        .apply(
            "synthetic-rotated-access".into(),
            Some("synthetic-rotated-refresh".into()),
            3600,
            future_rfc3339_after_secs(3600),
            None,
            None,
            None,
        )
        .unwrap();
    drop(attempt);
    let candidates = snapshot.resolve_candidates(None);
    assert_eq!(candidates.len(), 1);
    let payload = &candidates[0].payload;
    assert_eq!(payload.api_key, "synthetic-rotated-access");
    assert_eq!(
        payload.extra_body.as_ref().unwrap()["token"],
        "synthetic-old-access"
    );
    assert_eq!(
        plan(payload, json!({})).unwrap()["token"],
        "synthetic-rotated-access"
    );
    assert_eq!(
        snapshot.document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("synthetic-old-access")
    );
}
