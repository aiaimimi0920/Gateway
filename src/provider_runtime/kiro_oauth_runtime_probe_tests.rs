//! Explicit management generation keeps OAuth ownership; a generic probe never spends a call.
use crate::upstream::client::kiro_api_key_test_support as wire;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
#[path = "../../tests/console_contract_support/mod.rs"]
mod support;
const PROFILE: &str = "arn:aws:codewhisperer:eu-central-1:123456789012:profile/synthetic";
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/oauth-runtime-probe/probe";
fn document() -> Value {
    let context = json!({"auth_method":"builder_id","directory_region":"us-east-1",
        "start_url":"https://view.awsapps.com/start","client_id":"synthetic-client",
        "client_secret":"synthetic-secret","client_secret_expires_at":1893456000i64,
        "scopes":["codewhisperer:conversations"],"machine_id":"a".repeat(64),
        "runtime_target":{"base_url":"https://runtime.eu-central-1.kiro.dev","profile_arn":PROFILE}});
    let credential = |id: &str, enabled: bool| {
        json!({"id":id,"enabled":enabled,"api_key":format!("synthetic-access-{id}"),
        "kiro_oauth":context,"supported_models":["claude-sonnet-contract"]})
    };
    json!({"providers":[{"id":"oauth-runtime-probe","preset":"kiro","auth_mode":"kiro_oauth_runtime",
        "base_url":"https://runtime.eu-central-1.kiro.dev","responses_path":"/","extra_body":{"profileArn":PROFILE},
        "explicit_credentials_only":true,"supported_models":["probe-alias"],"model_map":{"probe-alias":"claude-sonnet-contract"},
        "credentials":[credential("exact",true),credential("other",true),credential("disabled",false)]}]})
}
#[tokio::test]
async fn kiro_oauth_runtime_management_probe_uses_exact_account_and_profile_without_key_header() {
    let server = wire::Server::start(
        200,
        "application/vnd.amazon.eventstream",
        wire::event_frame("OK"),
    )
    .await;
    let mut fixture =
        support::ConsoleStateFixture::new(serde_json::from_value(document()).unwrap(), false);
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = rquest::Client::builder().no_proxy().build().unwrap();
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = wire::within(
        &server.origin,
        Box::pin(support::send_console_json(
            &fixture.state,
            Method::POST,
            ENDPOINT,
            json!({"prompt":"hello","model":"probe-alias","credentialIds":["other"]}),
            Some(&grant),
            None,
        )),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    assert_eq!(body["result"]["results"][0]["status"], "passed", "{body}");
    assert!(body.to_string().contains("Kiro OAuth runtime"));
    let calls = server.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0["authorization"], "Bearer synthetic-access-other");
    assert!(!calls[0].0.contains_key("tokentype"));
    assert_eq!(calls[0].1["profileArn"], PROFILE);
    assert_eq!(
        calls[0].1["conversationState"]["currentMessage"]["userInputMessage"]["modelId"],
        "claude-sonnet-contract"
    );
    let response = wire::within(
        &server.origin,
        Box::pin(support::probe_credential(&fixture.state, &grant, "exact")),
    )
    .await;
    let report = support::parse_json(response).await;
    assert_eq!(report["result"]["status"], "unsupported", "{report}");
    // Partial options, a missing Grant, disabled accounts and unconfigured models never generate.
    for (value, grant, expected) in [
        (
            json!({"credentialIds":["exact"]}),
            Some(grant.as_str()),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"prompt":"hello","model":"probe-alias","credentialIds":["exact"]}),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            json!({"prompt":"hello","model":"probe-alias","credentialIds":["disabled"]}),
            Some(grant.as_str()),
            StatusCode::OK,
        ),
        (
            json!({"prompt":"hello","model":"foreign","credentialIds":["exact"]}),
            Some(grant.as_str()),
            StatusCode::OK,
        ),
    ] {
        let response = wire::within(
            &server.origin,
            Box::pin(support::send_console_json(
                &fixture.state,
                Method::POST,
                ENDPOINT,
                value,
                grant,
                None,
            )),
        )
        .await;
        assert_eq!(response.status(), expected);
    }
    assert_eq!(server.calls.lock().unwrap().len(), 1);
    server.stop().await;
}
