use super::*;
use crate::{
    console::document::validate_route_document,
    preset::{compile_provider_account, get_builtin_preset, AccountOverrides},
    protocol::canonical::EndpointKind,
};
use serde_json::json;

fn document() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../docs/providers/examples/kilo-free.routes.sample.json"
    ))
    .unwrap()
}

#[test]
fn config_accepts_anonymous_route_but_rejects_paid_or_overridden_settings() {
    let doc = document();
    assert!(validate_route_document(serde_json::from_value(doc.clone()).unwrap()).is_ok());
    for (field, value) in [
        ("api_key", json!("not-allowed")),
        (
            "base_url",
            json!("https://api.kilo.ai.example.org/api/gateway"),
        ),
        ("supported_models", json!(["paid"])),
        ("default_model", json!("paid")),
        ("credentials", json!([{"id":"unnecessary"}])),
        ("explicit_credentials_only", json!(true)),
        ("headers", json!({"Authorization":"not-allowed"})),
        ("responses_path", json!("/responses")),
        ("auth_mode", json!("bearer")),
    ] {
        let mut bad = doc.clone();
        bad["providers"][0][field] = value;
        assert!(
            validate_route_document(serde_json::from_value(bad).unwrap()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn kilo_addition_accepts_legacy_unprobed_catalogue_rows_but_not_invalid_evidence() {
    let mut doc = document();
    doc["providers"].as_array_mut().unwrap().insert(0, json!({
        "id": "legacy",
        "base_url": "https://fixture.test",
        "adapter": "openai_compatible",
        "credentials": [{
            "id": "legacy-account",
            "api_key": "fixture",
            "supported_models": ["sample-model"],
            "discovery": {
                "source_url": "https://fixture.test",
                "api_base": "https://fixture.test/v1",
                "protocol": "chat_completions",
                "models": ["sample-model", "bad\nrow"],
                "verified_models": ["sample-model"],
                "checked_at": "2026-10-26T04:00:00Z",
                "binding": crate::provider_discovery::binding("https://fixture.test", "fixture"),
            }
        }]
    }));
    // A UI-created provider carries these settings, but leaves out call_flow so
    // the anonymous preset keeps control of its transport.
    doc["providers"][1]["explicit_credentials_only"] = json!(false);
    doc["providers"][1]["credentials"] = json!([]);
    doc["providers"][1]["pool_min_size"] = json!(0);
    doc["providers"][1]["pool_target_size"] = json!(100);
    assert!(validate_route_document(serde_json::from_value(doc.clone()).unwrap()).is_ok());
    doc["providers"][0]["credentials"][0]["discovery"]["verified_models"] = json!(["bad\nrow"]);
    assert!(validate_route_document(serde_json::from_value(doc).unwrap()).is_err());
}

#[test]
fn final_plan_checks_free_model_and_strips_all_ambient_headers() {
    let payload = compile_provider_account(
        &get_builtin_preset(PRESET).unwrap(),
        &serde_json::from_value::<AccountOverrides>(json!({
            "base_url": BASE,
            "api_key": ""
        }))
        .unwrap(),
    );
    let mut plan = RequestPlan {
        method: rquest::Method::POST,
        url: CHAT.into(),
        query: Vec::new(),
        body: Some(json!({"model": MODEL, "messages": [], "stream": true})),
        response_kind: EndpointKind::ChatCompletions,
    };
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("Bearer caller"));
    headers.insert(
        "x-kilocode-organizationid",
        HeaderValue::from_static("caller-org"),
    );
    finalize_headers(&payload, &plan, &mut headers).unwrap();
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[ACCEPT], "text/event-stream");
    plan.body.as_mut().unwrap()["models"] = json!(["paid"]);
    assert!(finalize_headers(&payload, &plan, &mut headers).is_err());
    plan.body = Some(json!({"model": "paid", "messages": []}));
    assert!(finalize_headers(&payload, &plan, &mut headers).is_err());
    plan.body = Some(json!({"model": MODEL, "messages": []}));
    plan.url.push_str("?x=1");
    assert!(finalize_headers(&payload, &plan, &mut headers).is_err());
}

#[test]
fn shared_ip_rate_limit_is_terminal_and_retains_upstream_timing() {
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("120"));
    let error = rate_limit(&headers);
    assert_eq!(error.http_status, Some(429));
    assert_eq!(error.upstream_retry_after_ms, Some(120_000));
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
}
