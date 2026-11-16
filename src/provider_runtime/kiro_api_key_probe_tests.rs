//! Ordinary management grants and exact static-account generation; no real credential or AWS I/O.
use crate::upstream::client::kiro_api_key_test_support as wire;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
#[path = "../../tests/console_contract_support/mod.rs"]
mod support;
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/kiro-key-probe/probe";
fn document() -> Value {
    let credential = |id: &str, enabled: bool| json!({"id":id,"enabled":enabled,"api_key":format!("ksk_{id}"),"supported_models":["claude-sonnet-contract"]});
    json!({"providers":[{"id":"kiro-key-probe","preset":"kiro-api-key","base_url":"https://runtime.us-east-1.kiro.dev",
        "explicit_credentials_only":true,"supported_models":["probe-alias"],"model_map":{"probe-alias":"claude-sonnet-contract"},
        "credentials":[credential("exact",true),credential("other",true),credential("disabled",false)]}]})
}
fn run<F: Future<Output = ()> + 'static>(test: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(30), test())
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
fn fixture() -> support::ConsoleStateFixture {
    let mut fixture =
        support::ConsoleStateFixture::new(serde_json::from_value(document()).unwrap(), false);
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = rquest::Client::builder().no_proxy().build().unwrap();
    fixture
}
#[test]
fn kiro_api_key_management_probe_uses_grant_exact_account_and_ordinary_cli_sender() {
    run(exact);
}
async fn exact() {
    let server = wire::Server::start(
        200,
        "application/vnd.amazon.eventstream",
        wire::event_frame("OK ksk_other"),
    )
    .await;
    let fixture = fixture();
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = wire::within(
        &server.origin,
        support::send_console_json(
            &fixture.state,
            Method::POST,
            ENDPOINT,
            json!({"prompt":"合成问题","model":"probe-alias","credentialIds":["other"]}),
            Some(&grant),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = support::parse_json(response).await;
    assert_eq!(body["result"]["results"][0]["status"], "passed", "{body}");
    assert!(!body.to_string().contains("ksk_"));
    let calls = server.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    let (headers, body) = &calls[0];
    assert_eq!(headers["authorization"], "Bearer ksk_other");
    assert_eq!(headers["tokentype"], "API_KEY");
    let current = &body["conversationState"]["currentMessage"]["userInputMessage"];
    assert_eq!(current["content"], "合成问题");
    assert_eq!(current["origin"], "KIRO_CLI");
    assert_eq!(current["modelId"], "claude-sonnet-contract");
    server.stop().await;
}
#[test]
fn kiro_api_key_management_probe_no_explicit_request_grant_disabled_or_wrong_model_never_send() {
    run(rejections);
}
async fn rejections() {
    let server = wire::Server::start(
        200,
        "application/vnd.amazon.eventstream",
        wire::event_frame("OK"),
    )
    .await;
    let fixture = fixture();
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = wire::within(
        &server.origin,
        support::probe_credential(&fixture.state, &grant, "exact"),
    )
    .await;
    let body = support::parse_json(response).await;
    assert_eq!(body["result"]["status"], "unsupported", "{body}");
    for (credential, model, grant, expected) in [
        ("other", "probe-alias", None, StatusCode::FORBIDDEN),
        (
            "disabled",
            "probe-alias",
            Some(grant.as_str()),
            StatusCode::OK,
        ),
        (
            "exact",
            "unconfigured",
            Some(grant.as_str()),
            StatusCode::OK,
        ),
    ] {
        let response = wire::within(
            &server.origin,
            support::send_console_json(
                &fixture.state,
                Method::POST,
                ENDPOINT,
                json!({"prompt":"hello","model":model,"credentialIds":[credential]}),
                grant,
                None,
            ),
        )
        .await;
        assert_eq!(response.status(), expected);
        if expected == StatusCode::OK {
            let body = support::parse_json(response).await;
            assert_ne!(body["result"]["results"][0]["status"], "passed", "{body}");
        }
    }
    assert!(server.calls.lock().unwrap().is_empty());
    server.stop().await;
}
