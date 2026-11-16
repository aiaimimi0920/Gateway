//! Final sender checks must run after all header mutations, not only during planning.
use crate::{
    protocol::openai,
    routing::candidate::ProviderAccountPayload,
    upstream::{client::UpstreamClient, headers::build_inference_headers},
};
use rquest::header::{HeaderName, HeaderValue};
use serde_json::json;
#[cfg(feature = "line-nous-portal-oauth")]
mod transport;

#[test]
#[cfg(feature = "line-nous-portal-oauth")]
fn nous_final_sender_rejects_late_destination_and_credential_mutation() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "auth_mode":"nous_portal_oauth",
        "base_url":"https://inference-api.nousresearch.com/v1", "api_key":"synthetic-access"
    }))
    .unwrap();
    let request = openai::normalize_chat_completions(json!({
        "model":"org/model", "messages":[{"role":"user","content":"hello"}]
    }))
    .unwrap();
    let mut plan =
        UpstreamClient::build_request_plan(&payload, &request, "org/model", false).unwrap();
    let headers = build_inference_headers(&payload, &request, "org/model", None);
    let client = UpstreamClient::new(3);
    for name in [
        "cookie",
        "host",
        "proxy-authorization",
        "x-api-key",
        "api-key",
        "x-goog-api-key",
    ] {
        let mut bad = headers.clone();
        bad.insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_static("foreign"),
        );
        assert!(
            client.send_profiled_plan(&payload, &plan, bad).is_err(),
            "accepted late {name}"
        );
    }
    let mut bad = headers.clone();
    bad.insert("authorization", HeaderValue::from_static("Bearer foreign"));
    assert!(client.send_profiled_plan(&payload, &plan, bad).is_err());
    plan.url = "https://attacker.invalid/v1/chat/completions".into();
    assert!(client.send_profiled_plan(&payload, &plan, headers).is_err());
}
