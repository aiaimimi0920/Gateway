//! Exact management requests use the selected synthetic Key and the ordinary bound sender.
use crate::upstream::client::vertex_express_test_support as wire;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
#[path = "../../tests/console_contract_support/mod.rs"]
mod support;
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/express-probe/probe";
fn document() -> Value {
    let credential = |id: &str, enabled: bool| json!({"id":id,"enabled":enabled,"api_key":format!("synthetic_{id}"),"supported_models":["gemini-contract"]});
    json!({"providers":[{"id":"express-probe","preset":"vertex-express-api","base_url":crate::protocol::vertex_express::BASE_URL,
        "explicit_credentials_only":true,"supported_models":["probe-alias"],"model_map":{"probe-alias":"gemini-contract"},
        "credentials":[credential("exact",true),credential("other",true),credential("disabled",false)]}]})
}
fn run(test: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(Box::pin(async {
            tokio::time::timeout(Duration::from_secs(30), test)
                .await
                .unwrap();
        }));
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
fn reply(parts: Value) -> Vec<u8> {
    format!(
        "data: {}\n\n",
        json!({"candidates":[{"content":{"role":"model","parts":parts},"finishReason":"STOP"}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1,"totalTokenCount":3}})
    )
    .into_bytes()
}
#[test]
fn vertex_express_probe_model_identity_is_a_bounded_single_segment() {
    let payload = crate::routing::config::RouteConfigStore::from_document(
        serde_json::from_value(document()).unwrap(),
    )
    .unwrap();
    let snapshot = payload.snapshot();
    let payload = &snapshot.get_providers()[0].credential_pool[0].payload;
    assert!(crate::provider_model_id::valid_for_payload(
        &"A".repeat(2048),
        payload
    ));
    for model in [
        "models/foo",
        "x%2fsecret",
        "x#f",
        "x:generateContent",
        "中文",
    ] {
        assert!(
            !crate::provider_model_id::valid_for_payload(model, payload),
            "accepted {model}"
        );
    }
}
#[test]
fn vertex_express_generic_probe_sender_rejects_the_key_before_any_network() {
    run(async {
        // This guard must reject even a body-less generic probe; do not require Gemini JSON.
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = axum::Router::new().fallback(move || {
            let observed = observed.clone();
            async move {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                StatusCode::OK
            }
        });
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        let fixture =
            support::ConsoleStateFixture::new(serde_json::from_value(document()).unwrap(), false);
        let snapshot = fixture.state.route_config.snapshot();
        let payload = &snapshot.get_providers()[0].credential_pool[0].payload;
        let client = rquest::Client::builder().no_proxy().build().unwrap();
        let result = super::http_probe::send_probe_request(
            &client,
            rquest::Method::POST,
            origin,
            payload,
            super::http_probe::ProbeExpectation::HttpOk,
        )
        .await;
        stop.send(()).unwrap();
        task.await.unwrap();
        assert_eq!(
            result.unwrap_err().code.as_deref(),
            Some("invalid_vertex_express_api_key")
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    });
}
#[test]
fn vertex_express_management_probe_binds_exact_account_model_and_redacts_material() {
    run(async {
        let server =
            wire::Server::start(200, reply(json!([{ "text": "OK synthetic_other" }]))).await;
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
        assert!(
            body["result"]["results"][0]["probePoint"]
                .as_str()
                .unwrap()
                .contains("Vertex Express"),
            "{body}"
        );
        assert!(!body.to_string().contains("synthetic_"));
        let calls = server.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert!(calls[0]
            .0
            .contains("/models/gemini-contract:streamGenerateContent"));
        assert_eq!(calls[0].1["x-goog-api-key"], "synthetic_other");
        assert!(!calls[0].1.contains_key("authorization"));
        assert_eq!(calls[0].2["contents"][0]["parts"][0]["text"], "合成问题");
        server.stop().await;
    });
}
#[test]
fn vertex_express_management_probe_requires_grant_explicit_request_enabled_exact_model() {
    run(async {
        let server = wire::Server::start(200, reply(json!([{ "text": "OK" }]))).await;
        let fixture = fixture(&server);
        let grant = support::grant_console_secret_access(&fixture.state).await;
        let response = wire::within(
            &server.origin,
            support::probe_credential(&fixture.state, &grant, "exact"),
        )
        .await;
        let body = support::parse_json(response).await;
        assert_eq!(body["result"]["status"], "unsupported", "{body}");
        assert!(
            body["result"]["message"]
                .as_str()
                .unwrap()
                .contains("Vertex Express"),
            "{body}"
        );
        for (id, model, grant, status) in [
            ("other", "probe-alias", None, StatusCode::FORBIDDEN),
            ("other", "probe-alias", Some(grant.as_str()), StatusCode::OK),
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
            ("exact", "models/foo", Some(grant.as_str()), StatusCode::OK),
        ] {
            let origin = (id == "other" && grant.is_some()).then_some("https://different.invalid");
            let response = wire::within(
                &server.origin,
                support::send_console_json(
                    &fixture.state,
                    Method::POST,
                    ENDPOINT,
                    json!({"prompt":"hello","model":model,"credentialIds":[id]}),
                    grant,
                    origin,
                ),
            )
            .await;
            assert_eq!(
                response.status(),
                if origin.is_some() {
                    StatusCode::FORBIDDEN
                } else {
                    status
                }
            );
            if response.status() == StatusCode::OK {
                let body = support::parse_json(response).await;
                assert_ne!(body["result"]["results"][0]["status"], "passed", "{body}");
            }
        }
        assert!(server.calls.lock().unwrap().is_empty());
        server.stop().await;
    });
}
#[test]
fn vertex_express_management_probe_does_not_accept_tools_or_oversized_text_as_text_proof() {
    run(async {
        for parts in [
            json!([{ "text":"OK" },{"functionCall":{"name":"tool","args":{}}}]),
            json!([{ "text":"X".repeat(8193) }]),
        ] {
            let server = wire::Server::start(200, reply(parts)).await;
            let fixture = fixture(&server);
            let grant = support::grant_console_secret_access(&fixture.state).await;
            let response = wire::within(
                &server.origin,
                support::send_console_json(
                    &fixture.state,
                    Method::POST,
                    ENDPOINT,
                    json!({"prompt":"hello","model":"gemini-contract","credentialIds":["exact"]}),
                    Some(&grant),
                    None,
                ),
            )
            .await;
            let body = support::parse_json(response).await;
            assert_eq!(body["result"]["results"][0]["status"], "failed", "{body}");
            assert_eq!(server.calls.lock().unwrap().len(), 1);
            server.stop().await;
        }
    });
}
