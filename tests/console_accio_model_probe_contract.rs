//! Exact-account Accio tests must use the native dispatcher and require a completed answer.
#![cfg(feature = "line-accio-web-reverse-api")]
#[path = "accio_relay_contract/fixture.rs"]
mod fixture;
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use serde_json::json;
use support::*;

async fn probe(f: &ConsoleStateFixture, grant: Option<&str>, id: &str) -> axum::response::Response {
    send_console_json(
        &f.state,
        Method::POST,
        "/v1/internal/gateway/console/providers/accio-test/probe",
        json!({"prompt":"Explicit probe prompt","model":"probe-alias","credentialIds":[id]}),
        grant,
        None,
    )
    .await
}
#[test]
fn accio_probe_calls_only_the_selected_account_on_native_wire() {
    fixture::on_runtime(|| async {
        let server = fixture::Upstream::start(None).await;
        let f = ConsoleStateFixture::new(server.document(&["first", "good", "disabled"]), false);
        let grant = grant_console_secret_access(&f.state).await;
        let body = parse_json(probe(&f, Some(&grant), "good").await).await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["credentialId"], "good");
        assert_eq!(result["status"], "passed", "{body}");
        let calls = server.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "/api/adk/llm/generateContent");
        assert_eq!(calls[0].2["token"], "good-secret");
        assert_eq!(calls[0].2["utdid"], "good-device");
        assert_eq!(calls[0].2["model"], fixture::MODEL);
        assert!(calls[0].2.to_string().contains("Explicit probe prompt"));
        assert_eq!(calls[0].1["utdid"], "good-device");
        server.finish().await;
    });
}
#[test]
fn accio_probe_rejects_incomplete_empty_tool_and_error_answers_without_account_fallback() {
    fixture::on_runtime(|| async {
        let ids = [
            "empty",
            "garbage",
            "truncated",
            "mid-error",
            "split-error",
            "tail",
            "usage-only",
            "done-only",
            "length",
            "tool",
            "unauthorized",
        ];
        let server = fixture::Upstream::start(None).await;
        let f = ConsoleStateFixture::new(server.document(&ids), false);
        let grant = grant_console_secret_access(&f.state).await;
        for id in ids {
            let body = parse_json(probe(&f, Some(&grant), id).await).await;
            assert_eq!(
                body["result"]["results"][0]["status"], "failed",
                "{id}: {body}"
            );
            assert!(!body.to_string().contains("synthetic-private-upstream"));
        }
        let calls = server.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), ids.len());
        assert_eq!(
            calls
                .iter()
                .map(|c| c.2["token"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ids.map(|id| format!("{id}-secret"))
        );
        server.finish().await;
    });
}
#[test]
fn accio_probe_never_generates_without_options_grant_enabled_and_model_admission() {
    fixture::on_runtime(|| async {
        let server = fixture::Upstream::start(None).await;
        let f = ConsoleStateFixture::new(server.document(&["good", "disabled"]), false);
        assert_eq!(
            probe(&f, None, "good").await.status(),
            StatusCode::FORBIDDEN
        );
        let grant = grant_console_secret_access(&f.state).await;
        for body in [
            json!({"prompt":"Explicit probe prompt","model":"other","credentialIds":["good"]}),
            json!({"prompt":"Explicit probe prompt","model":"probe-alias","credentialIds":["disabled"]}),
        ] {
            let response = send_console_json(
                &f.state,
                Method::POST,
                "/v1/internal/gateway/console/providers/accio-test/probe",
                body,
                Some(&grant),
                None,
            )
            .await;
            let body = parse_json(response).await;
            assert_eq!(
                body["result"]["results"][0]["status"], "unsupported",
                "{body}"
            );
        }
        let body = parse_json(probe_credential(&f.state, &grant, "good").await).await;
        assert_eq!(body["result"]["status"], "unsupported");
        assert!(server.calls.lock().unwrap().is_empty());
        server.finish().await;
    });
}
#[test]
fn accio_probe_redacts_all_selected_material_from_a_valid_reply() {
    fixture::on_runtime(|| async {
        let server = fixture::Upstream::start(None).await;
        let f = ConsoleStateFixture::new(server.document(&["echo"]), false);
        let grant = grant_console_secret_access(&f.state).await;
        let body = parse_json(probe(&f, Some(&grant), "echo").await).await;
        assert_eq!(body["result"]["results"][0]["status"], "passed", "{body}");
        for secret in ["echo-secret", "echo-device", "echo-sg", "echo-refresh"] {
            assert!(!body.to_string().contains(secret), "{body}");
        }
        server.finish().await;
    });
}

#[test]
fn accio_matrix_persists_exact_account_and_validated_usage() {
    fixture::on_runtime(|| async {
        use neuro_gateway::{db, local_runtime::LocalRuntime};
        use std::sync::Arc;
        let server = fixture::Upstream::start(None).await;
        let mut f = ConsoleStateFixture::new(server.document(&["first", "good"]), false);
        let directory =
            std::env::temp_dir().join(format!("accio-probe-audit-{}", uuid::Uuid::new_v4()));
        let local = LocalRuntime::open(&directory).await.unwrap();
        Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local.clone());
        let grant = grant_console_secret_access(&f.state).await;
        let policy = json!({"automaticEnabled":false,"intervalMinutes":15,"modelSelection":"all","models":[],
        "cases":[{"id":"ok","name":"OK","prompt":"Explicit matrix prompt","expectedAnswer":"OK 你好","difficulty":1,"enabled":true}]});
        let body = parse_json(
            send_console_json(
                &f.state,
                Method::POST,
                "/v1/internal/gateway/console/providers/accio-test/probe",
                json!({"testPlan":policy,"scope":{"kind":"account","id":"good"}}),
                Some(&grant),
                None,
            )
            .await,
        )
        .await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["credentialId"], "good");
        assert_eq!(result["status"], "passed", "{body}");
        assert_eq!(result["assessment"]["cases"][0]["correct"], true);
        assert_eq!(server.calls.lock().unwrap().len(), 1);
        let audits = local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].provider_account_id.as_deref(), Some("accio-test"));
        assert_eq!(audits[0].resolved_model.as_deref(), Some(fixture::MODEL));
        assert_eq!(audits[0].status, "completed");
        assert_eq!(audits[0].prompt_tokens, Some(5));
        assert_eq!(audits[0].completion_tokens, Some(2));
        assert_eq!(audits[0].total_tokens, Some(9));
        assert_eq!(audits[0].cache_creation_input_tokens, Some(3));
        assert_eq!(audits[0].cache_read_input_tokens, Some(4));
        assert_eq!(
            audits[0].route_trace.as_ref().unwrap()["realCredentialRef"],
            "good"
        );
        server.finish().await;
        drop(f);
        local.close().await;
    });
}
