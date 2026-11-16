//! Ordinary management authentication and exact credential targeting, using synthetic loopback SSE.
use crate::upstream::perplexity_web::test_support as wire;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
#[path = "../../tests/console_contract_support/mod.rs"]
mod support;
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/perplexity-probe/probe";
fn document() -> Value {
    let credential = |id: &str, token: &str, enabled: bool| {
        json!({"id":id,"enabled":enabled,"api_key":token,
        "headers":{"User-Agent":format!("{id}-UA")},"session_auth":{"transport":"cookie","primaryCookieName":"__Secure-next-auth.session-token"}})
    };
    json!({"providers":[{"id":"perplexity-probe","preset":"perplexity-web","base_url":"https://www.perplexity.ai",
        "explicit_credentials_only":true,"supported_models":["probe-alias"],"model_map":{"probe-alias":"turbo"},
        "credentials":[credential("exact","synthetic-exact",true),credential("other","synthetic-other",true),credential("disabled","synthetic-disabled",false)]}]})
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
fn fixture(server: &wire::Server) -> support::ConsoleStateFixture {
    let mut fixture =
        support::ConsoleStateFixture::new(serde_json::from_value(document()).unwrap(), false);
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = server.client();
    fixture
}
#[test]
fn perplexity_web_probe_uses_normal_grant_and_exact_resolved_account_without_api_defaults() {
    run(exact);
}
async fn exact() {
    let server = wire::Server::start(wire::completed(), 200, false).await;
    let fixture = fixture(&server);
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
    assert!(!body.to_string().contains("synthetic-"));
    let calls = server.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].headers["cookie"],
        "__Secure-next-auth.session-token=synthetic-other"
    );
    assert_eq!(calls[0].headers["user-agent"], "other-UA");
    assert_eq!(calls[0].body["query_str"], "合成问题");
    assert_eq!(calls[0].body["params"]["model_preference"], "turbo");
}
#[test]
fn perplexity_web_probe_no_explicit_request_disabled_unknown_model_or_no_grant_never_send() {
    run(rejections);
}
async fn rejections() {
    let server = wire::Server::start(wire::completed(), 200, false).await;
    let fixture = fixture(&server);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = wire::within(
        &server.origin,
        support::probe_credential(&fixture.state, &grant, "exact"),
    )
    .await;
    let body = support::parse_json(response).await;
    assert_eq!(body["result"]["status"], "unsupported", "{body}");
    for (credential, model, grant, expected_status) in [
        ("exact", "probe-alias", None, StatusCode::FORBIDDEN),
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
        assert_eq!(response.status(), expected_status);
        if response.status() == StatusCode::OK {
            let body = support::parse_json(response).await;
            assert_ne!(body["result"]["results"][0]["status"], "passed", "{body}");
        }
    }
    assert_eq!(server.attempts(), 0);
}
