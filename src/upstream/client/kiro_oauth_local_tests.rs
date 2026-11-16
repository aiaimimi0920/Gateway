//! Normal Kiro OAuth management -> real TCP public protocols -> durable local balances and rebuilds.
use super::{
    kiro_api_key_local_tests::{config, remaining, run, LocalServer},
    kiro_api_key_test_support as wire,
};
use crate::{console::kiro_oauth::test_support as issuer, db};
use serde_json::{json, Value};
use std::time::Duration;
mod browser_fixture;
mod failures;
mod fixture;
mod flow;
mod integrity;
use fixture::Fixture;

const PROVIDER: &str = "kiro-oauth";
const ALIAS: &str = "public-kiro";
const MODEL: &str = "Exact.Model/Version";
const PROFILE: &str = "arn:aws:codewhisperer:eu-central-1:123456789012:profile/local";
const ROUTE: &str = "/v1/internal/gateway/console/route-config";
const AUTH: &str = "/v1/internal/gateway/console/kiro-auth-sessions";

fn document() -> Value {
    json!({"providers":[{"id":PROVIDER,"preset":"kiro","auth_mode":"kiro_oauth_runtime",
        "base_url":"https://runtime.eu-central-1.kiro.dev","responses_path":"/","extra_body":{"profileArn":PROFILE},
        "explicit_credentials_only":true,"supported_models":[ALIAS],"model_map":{ALIAS:MODEL},
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"kiro"},
        "credentials":[]}],"model_routes":[{"pattern":ALIAS,"provider_ids":[PROVIDER]}]})
}
fn body(endpoint: &str, stream: bool) -> Value {
    match endpoint {
        "responses" => json!({"model":ALIAS,"input":"local question","stream":stream}),
        "messages" => {
            json!({"model":ALIAS,"messages":[{"role":"user","content":"local question"}],"max_tokens":32,"stream":stream})
        }
        _ => {
            json!({"model":ALIAS,"messages":[{"role":"user","content":"local question"}],"stream":stream})
        }
    }
}
fn assert_wire(upstream: &wire::Server, expected: usize) {
    let calls = upstream.calls.lock().unwrap();
    assert_eq!(calls.len(), expected);
    assert_eq!(upstream.attempts(), expected);
    for (headers, body) in calls.iter() {
        assert_eq!(headers["authorization"], "Bearer synthetic-console-access");
        assert!(!headers.contains_key("tokentype") && !headers.contains_key("cookie"));
        assert_eq!(
            headers["x-amz-target"],
            crate::protocol::kiro_api_key::TARGET
        );
        assert_eq!(headers["amz-sdk-request"], "attempt=1; max=1");
        assert_ne!(headers["user-agent"], "caller-must-not-forward");
        assert_eq!(body["profileArn"], PROFILE);
        let current = &body["conversationState"]["currentMessage"]["userInputMessage"];
        assert_eq!(current["modelId"], MODEL);
        assert_eq!(current["origin"], "KIRO_CLI");
        assert_eq!(current["content"], "local question");
    }
}
