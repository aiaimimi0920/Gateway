//! Public normalized-material APIs and real route compilation, using synthetic data only.
use neuro_gateway::{
    protocol::{call_flow, perplexity_web::material as product},
    routing::{
        candidate::ProviderAccountPayload,
        config::{RouteConfigStore, RouteConfigYaml},
    },
    upstream::perplexity_web::Session,
};
use serde_json::{json, Value};

fn payload_value() -> Value {
    json!({"adapter":product::ADAPTER,"auth_mode":product::AUTH_MODE,"base_url":product::BASE_URL,
        "api_key":"synthetic-session", "headers":{"User-Agent":"synthetic-agent",
        "Cookie":"__Secure-next-auth.session-token=synthetic-session; cf_clearance=synthetic-clearance"},
        "session_auth":{"transport":"cookie","primaryCookieName":product::SESSION_COOKIE},
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"perplexity_web"}})
}
fn payload(value: Value) -> ProviderAccountPayload {
    serde_json::from_value(value).unwrap()
}
fn compile(provider: Value) -> anyhow::Result<RouteConfigStore> {
    let document: RouteConfigYaml =
        serde_json::from_value(json!({"providers":[provider],"model_routes":[]})).unwrap();
    RouteConfigStore::from_document(document)
}
fn provider() -> Value {
    json!({"id":"operator-name", "adapter":product::ADAPTER, "auth_mode":product::AUTH_MODE,
        "base_url":product::BASE_URL, "explicit_credentials_only":true, "credentials":[]})
}

#[test]
fn perplexity_web_material_exact_shape_reaches_preparation_and_session() {
    let value = payload_value();
    let p = payload(value.clone());
    let prepared = call_flow::prepare(&p).unwrap();
    assert_eq!(prepared.executor, product::ADAPTER);
    assert!(prepared.executor_owns_fingerprint);
    assert_eq!(
        serde_json::to_value(prepared.payload.as_ref()).unwrap()["headers"],
        value["headers"]
    );
    assert!(Session::from_payload(&p).is_ok());
    let mut token_only = value;
    token_only["headers"]
        .as_object_mut()
        .unwrap()
        .remove("Cookie");
    assert!(Session::from_payload(&payload(token_only)).is_ok());
}

#[test]
fn perplexity_web_material_wrong_products_and_late_identity_overrides_fail_closed() {
    for (pointer, replacement) in [
        ("/adapter", json!("openai_compatible")),
        ("/auth_mode", json!("bearer")),
        ("/base_url", json!("https://api.perplexity.ai")),
        ("/base_url", json!("http://www.perplexity.ai")),
        ("/base_url", json!("https://user@www.perplexity.ai")),
        ("/base_url", json!("https://www.perplexity.ai/?leak")),
        ("/call_flow/fingerprint_adapter", json!("codex_cli")),
        ("/call_flow/call_adapter", json!("openai")),
        ("/call_flow/special_adapter", json!("none")),
        ("/call_flow/special_adapter", json!("grok_web")),
        ("/session_auth/primaryCookieName", json!("sso")),
        ("/session_auth/transport", json!("bearer")),
        (
            "/headers/Cookie",
            json!("__Secure-next-auth.session-token=other"),
        ),
    ] {
        let mut value = payload_value();
        *value.pointer_mut(pointer).unwrap() = replacement;
        let p = payload(value);
        assert!(call_flow::prepare(&p).is_err(), "{pointer}");
        assert!(Session::from_payload(&p).is_err(), "{pointer}");
    }
    for (key, value) in [
        ("auth_token", json!("private-secondary")),
        ("extra_body", json!({"token":"private-body"})),
        ("runtime_state_object_key", json!("profile")),
        ("messages_path", json!("/messages")),
        ("default_model", json!("sonar")),
    ] {
        let mut v = payload_value();
        v[key] = value;
        let error = Session::from_payload(&payload(v)).err().expect(key);
        assert_eq!(
            error.code.as_deref(),
            Some("perplexity_web_material_invalid")
        );
        assert!(!error.retryable);
        assert!(!error.message.contains("private") && !error.message.contains("attacker"));
    }
}

#[test]
fn perplexity_web_material_rejects_non_normalized_or_ambiguous_cookie_headers() {
    for cookie in ["cf_clearance=x", "__Secure-next-auth.session-token.0=x", "__Secure-next-auth.session-token=synthetic-session; tracking=x",
        "__Secure-next-auth.session-token=synthetic-session; __Secure-next-auth.session-token=synthetic-session",
        "__Secure-next-auth.session-token=synthetic-session; cf_clearance=x; cf_clearance=y"] {
        let mut v = payload_value(); v["headers"]["Cookie"] = json!(cookie);
        assert!(Session::from_payload(&payload(v)).is_err());
    }
    for headers in [
        json!({"User-Agent":"a","user-agent":"b"}),
        json!({"User-Agent":"a","Cookie":"x","cookie":"y"}),
        json!({"User-Agent":"bad\nagent"}),
        json!({"User-Agent":"a", "Authorization":"private-api"}),
        json!({}),
    ] {
        let mut v = payload_value();
        v["headers"] = headers;
        assert!(Session::from_payload(&payload(v)).is_err());
    }
}

#[test]
fn perplexity_web_material_expiry_is_not_extended_or_treated_as_refresh() {
    let mut v = payload_value();
    v["expires_at"] = json!("2020-01-01T00:00:00Z");
    v["session_auth"]["expiresAt"] = v["expires_at"].clone();
    let p = payload(v.clone());
    assert!(
        product::validate(&p, product::ADAPTER).is_ok(),
        "expired material remains storable"
    );
    let error = Session::from_payload(&p).err().unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("perplexity_web_session_expired")
    );
    v["expires_at"] = json!("2099-01-01T00:00:00Z");
    assert!(
        Session::from_payload(&payload(v.clone())).is_err(),
        "two expiry owners must match"
    );
    v["session_auth"]["expiresAt"] = v["expires_at"].clone();
    assert!(Session::from_payload(&payload(v.clone())).is_ok());
    v["expires_at"] = json!("invalid");
    v["session_auth"]["expiresAt"] = json!("invalid");
    assert!(Session::from_payload(&payload(v)).is_err());
}

#[test]
fn perplexity_web_material_configuration_keeps_empty_and_explicit_accounts_separate() {
    let store = compile(provider()).unwrap();
    assert!(store.resolve_candidates(Some("turbo")).is_empty());
    assert!(store.account_group_inventory().accounts.is_empty());
    let mut v = provider();
    let p = payload_value();
    v["credentials"] = json!([{"id":"a","api_key":p["api_key"],"headers":p["headers"],"session_auth":p["session_auth"]},
        {"id":"b","enabled":false}]);
    let store = compile(v.clone()).unwrap();
    let providers = store.get_providers();
    assert_eq!(providers[0].credential_pool.len(), 2);
    assert_eq!(providers[0].credential_pool[0].id, "a");
    assert_eq!(providers[0].credential_pool[1].id, "b");
    assert!(providers[0].credential_pool[1].payload.api_key.is_empty());
    assert!(!providers[0].credential_pool[1].enabled);
    for (key, value) in [
        ("refresh_token", json!("private-refresh")),
        ("refresh_endpoint", json!("https://attacker.invalid")),
        ("refresh_client_id", json!("other-client")),
    ] {
        let mut changed = v.clone();
        changed["credentials"][0][key] = value;
        assert!(compile(changed).is_err(), "{key}");
    }
    for (key, value) in [
        ("api_key", json!("shared")),
        ("headers", json!({"User-Agent":"shared"})),
        ("explicit_credentials_only", json!(false)),
    ] {
        let mut changed = provider();
        changed[key] = value;
        assert!(compile(changed).is_err(), "{key}");
    }
    v["credentials"][0]["base_url"] = json!("https://api.perplexity.ai");
    assert!(compile(v).is_err());
    let mut mixed = provider();
    mixed["credentials"] = json!([{"id":"web","enabled":false},
        {"id":"api","api_key":"synthetic-api", "base_url":"https://api.perplexity.ai", "auth_mode":"bearer"}]);
    assert!(
        compile(mixed).is_err(),
        "one Web pool cannot contain an API identity"
    );
}
