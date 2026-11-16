//! Privacy is monotonic across preflight skips, payload refresh and native protocol plans.
use super::*;
use serde_json::json;
use std::collections::HashMap;

fn payload(endpoint: &str) -> ProviderAccountPayload {
    let doc = json!({"providers":[{"id":"p","preset":PRESET,"base_url":BASE,"explicit_credentials_only":true,
        "credentials":[{"id":"c","api_key":"synthetic-key","supported_models":["upstream"],
        "model_catalog":{"sourceUrl":SOURCE,"checkedAt":"2026-11-13T00:00:00Z","models":["upstream"],
        "supportedEndpoints":{"upstream":[endpoint]}}}]}]});
    let mut p = crate::routing::config::RouteConfigStore::from_document(
        serde_json::from_value(doc).unwrap(),
    )
    .unwrap()
    .snapshot()
    .select_credential_probe_target("c")
    .unwrap()
    .payload;
    // Match production protocol selection; a probe target alone has not selected a wire path.
    if endpoint == "/responses" {
        p.responses_path = Some(endpoint.into());
        p.chat_completions_path = None;
    } else if endpoint == "/messages" {
        p.messages_path = Some(endpoint.into());
        p.chat_completions_path = None;
    }
    p
}

#[test]
fn commandcode_zdr_rejects_header_override_conflict_and_opt_out() {
    for headers in [
        json!({"x-cmd-zdr":"0"}),
        json!({"x-cmd-zdr":" 1"}),
        json!({"x-cmd-zdr":"1","X-Cmd-Zdr":"0"}),
        json!({"authorization":"foreign"}),
        json!({"x-cmd-zdr":"1","cookie":"foreign"}),
    ] {
        let mut p = payload("/chat/completions");
        p.headers = serde_json::from_value(headers).unwrap();
        assert!(validate(&p, p.canonical_adapter()).is_err());
    }
    assert!(zdr::configured(&HashMap::from([("X-Cmd-Zdr".into(), "1".into())])).unwrap());
    assert!(zdr::configured(&HashMap::from([
        ("X-Cmd-Zdr".into(), "1".into()),
        ("x-cmd-zdr".into(), "1".into())
    ]))
    .unwrap());
}

#[tokio::test]
async fn commandcode_zdr_classifies_only_fixed_upstream_code_without_reflecting_secrets() {
    let app = axum::Router::new().route(
        "/",
        axum::routing::get(|| async {
            (
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                axum::Json(json!({
                    "error":{"code":"cmd_zdr_no_providers","message":"synthetic-private-secret"}
                })),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let response = rquest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap()
        .get(url)
        .send()
        .await
        .unwrap();
    let error = zdr::upstream_error(response).await;
    assert_eq!(error.code.as_deref(), Some("cmd_zdr_no_providers"));
    assert_eq!(error.http_status, Some(422));
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!error.to_string().contains("synthetic-private-secret"));
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[test]
fn commandcode_zdr_latches_before_skips_and_survives_refresh_without_cross_product_downgrade() {
    let mut first = payload("/chat/completions");
    first.headers.insert(zdr::HEADER.into(), "1".into());
    let mut request = HashMap::new();
    zdr::latch(&mut request, &first).unwrap();
    // The first account is skipped before sending; another account lacks its setting.
    let mut refreshed = payload("/chat/completions");
    zdr::apply(&mut request, &mut refreshed).unwrap();
    assert_eq!(refreshed.headers[zdr::HEADER], "1");
    let mut foreign = refreshed.clone();
    foreign.auth_mode = None;
    foreign.headers.clear();
    let error = zdr::latch(&mut request, &foreign).unwrap_err();
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert_eq!(request[zdr::HEADER], "1");
    assert!(zdr::apply(&mut request, &mut foreign).is_err());
    assert!(foreign.headers.is_empty());
    // Existing non-ZDR routes are unaffected.
    zdr::latch(&mut HashMap::new(), &foreign).unwrap();
}

#[test]
fn commandcode_zdr_checks_final_native_responses_tools_without_restricting_messages() {
    let mut p = payload("/responses");
    p.headers.insert(zdr::HEADER.into(), "1".into());
    for kind in [
        "function",
        "custom",
        "local_shell",
        "web_search",
        "file_search",
        "computer",
        "future_tool",
    ] {
        let raw = json!({"model":"alias","input":"test","tools":[{
            "type":kind,"name":"lookup","parameters":{"type":"object"}
        }]});
        let req = crate::protocol::responses::normalize_responses(raw).unwrap();
        assert_eq!(
            runtime::plan(&p, &req, "upstream", false).is_ok(),
            matches!(kind, "function" | "custom" | "local_shell"),
            "{kind}"
        );
    }
    let mut p = payload("/messages");
    p.headers.insert(zdr::HEADER.into(), "1".into());
    let req = crate::protocol::anthropic::normalize_messages(native_request_tests::raw("messages"))
        .unwrap();
    assert!(runtime::plan(&p, &req, "upstream", false).is_ok());
}
