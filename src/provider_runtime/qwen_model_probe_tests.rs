//! Exact-account management probes use synthetic material; a trap blocks real upstream egress.
use axum::http::{Method, StatusCode};
use serde_json::json;
use std::{future::Future, sync::Arc, time::Duration};

#[path = "../../tests/console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

const BASE: &str = "https://workspace-a.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1";
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/qwen-probe/probe";

fn document() -> serde_json::Value {
    json!({"providers":[{
        "id":"qwen-probe", "preset":"qwen-official-api", "base_url":BASE,
        "call_flow":{"call_adapter":"openai","fingerprint_adapter":"codex_cli","special_adapter":"none"},
        "explicit_credentials_only":true, "supported_models":["probe-alias"],
        "model_map":{"probe-alias":"qwen-synthetic"},
        "credentials":[{"id":"exact", "api_key":"synthetic-exact-key", "supported_models":["qwen-synthetic"]},
            {"id":"empty", "api_key":"", "supported_models":["qwen-synthetic"]},
            {"id":"disabled", "enabled":false, "api_key":"synthetic-disabled-key"},
            {"id":"other", "api_key":"synthetic-other-key", "base_url":BASE.replace("workspace-a","workspace-b"), "supported_models":["probe-alias"]}]
    }]})
}
fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(serde_json::from_value(document()).unwrap(), false)
}

fn run<F: Future<Output = ()> + 'static>(test: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(test());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn trap(fixture: &mut ConsoleStateFixture) -> tokio::net::TcpListener {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    // An old implementation may try HTTPS CONNECT, but this local socket never forwards it.
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = rquest::Client::builder()
        .no_proxy()
        .proxy(rquest::Proxy::all(format!("http://{}", listener.local_addr().unwrap())).unwrap())
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    listener
}

#[test]
fn qwen_probe_without_request_is_unsupported_without_models_or_generation() {
    run(no_request);
}
async fn no_request() {
    let mut fixture = fixture();
    let listener = trap(&mut fixture).await;
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = support::probe_credential(&fixture.state, &grant, "exact").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(body["result"]["status"], "unsupported");
    assert!(body["result"]["probePoint"]
        .as_str()
        .unwrap()
        .contains("explicit"));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[test]
fn qwen_probe_empty_key_is_rejected_before_transport() {
    run(empty_key);
}
async fn empty_key() {
    let mut fixture = fixture();
    let listener = trap(&mut fixture).await;
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        ENDPOINT,
        json!({"prompt":"Reply OK", "model":"probe-alias", "credentialIds":["empty"]}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(body["result"]["results"][0]["status"], "failed");
    assert!(body["result"]["results"][0]["message"]
        .as_str()
        .unwrap()
        .contains("not sent"));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

async fn with_transport<T>(origin: &str, future: impl Future<Output = T>) -> T {
    crate::request_wait::transport::within(
        false,
        super::qwen_model_probe::test_transport::within(origin, future),
    )
    .await
}

#[path = "qwen_model_probe_cancellation_tests.rs"]
mod cancellation;
#[path = "qwen_model_probe_management_tests.rs"]
mod management;
#[path = "qwen_model_probe_response_tests.rs"]
mod responses;
#[path = "qwen_model_probe_test_support.rs"]
mod wire;
