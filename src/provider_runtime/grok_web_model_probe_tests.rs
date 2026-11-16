//! Real management grant/revision/credential selection, synthetic current WS contract.
use crate::upstream::grok_web::test_support as wire;
use axum::http::{Method, StatusCode};
use serde_json::json;
use std::{sync::Arc, time::Duration};
#[path = "../../tests/console_contract_support/mod.rs"]
mod support;
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/grok-probe/probe";
fn run(test: impl std::future::Future<Output = ()> + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(30), test)
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
fn fixture(server: &wire::Server) -> support::ConsoleStateFixture {
    let credential = |id: &str, enabled: bool| json!({"id":id,"enabled":enabled,"api_key":format!("synthetic-{id}"),"headers":{"User-Agent":format!("{id}-UA")}});
    let document = json!({"providers":[{"id":"grok-probe","preset":"grok","base_url":"https://grok.com",
        "explicit_credentials_only":true,"supported_models":["probe-alias"],"model_map":{"probe-alias":"grok-chat-expert"},
        "call_flow":{"special_adapter":"grok_web","fingerprint_adapter":"session"},
        "credentials":[credential("exact",true),credential("other",true),credential("disabled",false)]}]});
    let mut fixture =
        support::ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), false);
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = server.client();
    fixture
}
#[test]
fn grok_web_probe_uses_normal_grant_and_only_selected_account_mode() {
    run(async {
        let server = wire::Server::start(Default::default()).await;
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
        assert_eq!(calls.len(), 2);
        for call in calls {
            assert_eq!(
                call.headers["cookie"],
                "sso=synthetic-other; sso-rw=synthetic-other"
            );
            assert_eq!(call.headers["user-agent"], "other-UA");
        }
        assert_eq!(server.events()[0]["event"]["session"]["model"], "expert");
        drop(fixture);
        server.finish().await;
    });
}
#[test]
fn grok_web_probe_no_explicit_request_disabled_unknown_model_or_grant_never_send() {
    run(async {
        let server = wire::Server::start(Default::default()).await;
        let fixture = fixture(&server);
        let grant = support::grant_console_secret_access(&fixture.state).await;
        let response = wire::within(
            &server.origin,
            support::probe_credential(&fixture.state, &grant, "exact"),
        )
        .await;
        let body = support::parse_json(response).await;
        assert_eq!(body["result"]["status"], "unsupported", "{body}");
        for (credential, model, grant, status) in [
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
            assert_eq!(response.status(), status);
            if status == StatusCode::OK {
                let body = support::parse_json(response).await;
                assert_ne!(body["result"]["results"][0]["status"], "passed", "{body}");
            }
        }
        assert!(server.calls().is_empty());
    });
}
