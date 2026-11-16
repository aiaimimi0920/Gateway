//! Console generation must use the exact Bedrock credential and validate native completion.
#[path = "console_bedrock_model_probe_contract/fixture.rs"]
mod fixture;
#[path = "console_bedrock_model_probe_contract/lifetime.rs"]
mod lifetime;
#[path = "console_contract_support/mod.rs"]
mod support;

use axum::http::{Method, StatusCode};
use neuro_gateway::{db, local_runtime::LocalRuntime};
use serde_json::{json, Value};
use std::sync::Arc;
use support::*;

async fn probe(f: &ConsoleStateFixture, grant: Option<&str>, id: &str) -> axum::response::Response {
    send_console_json(
        &f.state,
        Method::POST,
        "/v1/internal/gateway/console/providers/bedrock-test/probe",
        json!({"prompt":"Reply OK","model":"probe-alias","credentialIds":[id]}),
        grant,
        None,
    )
    .await
}

#[tokio::test]
async fn bedrock_native_probe_uses_exact_account_and_persists_validated_usage() {
    let upstream = fixture::Upstream::start().await;
    let mut f =
        ConsoleStateFixture::new(upstream.document(&["good", "echo", "unauthorized"]), false);
    let directory =
        std::env::temp_dir().join(format!("bedrock-model-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&f.state).await;
    for (id, expected, needle) in [
        ("good", "passed", "Reply: OK"),
        ("echo", "passed", "[redacted]"),
        ("unauthorized", "failed", "HTTP 401"),
    ] {
        let response = probe(&f, Some(&grant), id).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["credentialId"], id);
        assert_eq!(result["providerId"], "bedrock-test");
        assert_eq!(result["status"], expected, "{body}");
        assert!(result["probePoint"]
            .as_str()
            .unwrap()
            .contains("ConverseStream"));
        assert!(
            result["message"].as_str().unwrap().contains(needle),
            "{body}"
        );
        assert!(!body.to_string().contains("-secret"));
    }
    assert_eq!(
        *upstream.calls.lock().unwrap(),
        ["good-secret", "echo-secret", "unauthorized-secret"]
    );
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 3);
    for audit in &audits {
        assert_eq!(audit.provider_account_id.as_deref(), Some("bedrock-test"));
        assert_eq!(audit.resolved_model.as_deref(), Some(fixture::MODEL));
        assert_eq!(audit.protocol_family, "bedrock_converse");
        assert!(audit.stream);
        if audit.status == "completed" {
            assert_eq!(audit.prompt_tokens, Some(5));
            assert_eq!(audit.completion_tokens, Some(2));
            assert_eq!(audit.total_tokens, Some(7));
        } else {
            assert_eq!(audit.status, "failed");
        }
    }
    let states = local
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    for id in ["good", "echo"] {
        assert!(states
            .iter()
            .any(|s| s.provider_credential_ref.as_deref() == Some(id)
                && s.model == fixture::MODEL
                && s.status == "active"));
    }
    upstream.finish().await;
    drop(f);
    local.close().await;
    // Keep the isolated SQLite evidence; no external directory cleanup is needed.
}

#[tokio::test]
async fn bedrock_probe_rejects_incomplete_or_foreign_wire_without_fallback() {
    let ids = [
        "badcrc",
        "badprelude",
        "missing-usage",
        "missing-stop",
        "truncated",
        "extra-tail",
        "empty",
        "length",
        "filtered",
        "oversized",
        "exception",
        "sse",
        "json",
        "forbidden",
        "limited",
        "tool-text",
        "duplicate-usage",
        "unknown-stop",
        "long-text",
    ];
    let upstream = fixture::Upstream::start().await;
    let f = ConsoleStateFixture::new(upstream.document(&ids), false);
    let grant = grant_console_secret_access(&f.state).await;
    for id in ids {
        let response = probe(&f, Some(&grant), id).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(
            body["result"]["results"][0]["status"], "failed",
            "{id}: {body}"
        );
        assert!(!body.to_string().contains("-secret"), "{id}: {body}");
    }
    assert_eq!(
        *upstream.calls.lock().unwrap(),
        ids.map(|id| format!("{id}-secret"))
    );
    upstream.finish().await;
}

#[tokio::test]
async fn bedrock_probe_admission_keeps_grant_disabled_model_and_no_options_boundaries() {
    let upstream = fixture::Upstream::start().await;
    let f = ConsoleStateFixture::new(upstream.document(&["good", "disabled"]), false);
    assert_eq!(
        probe(&f, None, "good").await.status(),
        StatusCode::FORBIDDEN
    );
    let grant = grant_console_secret_access(&f.state).await;
    for body in [
        json!({"prompt":"Reply OK","model":"other","credentialIds":["good"]}),
        json!({"prompt":"Reply OK","model":"probe-alias","credentialIds":["disabled"]}),
    ] {
        let result = parse_json(
            send_console_json(
                &f.state,
                Method::POST,
                "/v1/internal/gateway/console/providers/bedrock-test/probe",
                body,
                Some(&grant),
                None,
            )
            .await,
        )
        .await;
        assert_eq!(result["result"]["results"][0]["status"], "unsupported");
    }
    let result = parse_json(probe_credential(&f.state, &grant, "good").await).await;
    assert_eq!(result["result"]["status"], "unsupported");
    assert!(upstream.calls.lock().unwrap().is_empty());
    upstream.finish().await;
}

fn policy() -> Value {
    json!({"automaticEnabled":false,"intervalMinutes":15,"modelSelection":"all","models":[],
        "cases":[{"id":"ok","name":"OK","prompt":"Reply OK","expectedAnswer":"OK","difficulty":1,"enabled":true}]})
}

#[tokio::test]
async fn bedrock_probe_matrix_runs_configured_model_on_only_selected_credential() {
    let upstream = fixture::Upstream::start().await;
    let mut f = ConsoleStateFixture::new(upstream.document(&["good", "echo"]), false);
    let directory =
        std::env::temp_dir().join(format!("bedrock-matrix-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&f.state).await;
    let response = send_console_json(
        &f.state,
        Method::POST,
        "/v1/internal/gateway/console/providers/bedrock-test/probe",
        json!({"testPlan":policy(),"scope":{"kind":"account","id":"good"}}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    let result = &body["result"]["results"][0];
    assert_eq!(body["result"]["totalCount"], 1);
    assert_eq!(result["credentialId"], "good");
    assert_eq!(result["status"], "passed", "{body}");
    assert_eq!(result["assessment"]["cases"][0]["correct"], true);
    assert_eq!(result["assessment"]["cases"][0]["model"], fixture::MODEL);
    assert_eq!(*upstream.calls.lock().unwrap(), ["good-secret"]);
    upstream.finish().await;
    drop(f);
    local.close().await;
}

#[tokio::test]
async fn bedrock_probe_redacts_overlapping_iam_alias_values() {
    let upstream = fixture::Upstream::start().await;
    let mut document = upstream.document(&["iam"]);
    let mut value = serde_json::to_value(&document).unwrap();
    value["providers"][0]["credentials"][0]["api_key"] = json!("");
    value["providers"][0]["credentials"][0]["extra_body"] = json!({
        "AWS-ACCESS-KEY-ID":"synthetic", "AWS_SECRET_ACCESS_KEY":"synthetic-long-secret",
        "aws-session-token":"synthetic-long-session", "region":"us-east-1"});
    document = serde_json::from_value(value).unwrap();
    let f = ConsoleStateFixture::new(document, false);
    let grant = grant_console_secret_access(&f.state).await;
    let body = parse_json(probe(&f, Some(&grant), "iam").await).await;
    let result = &body["result"]["results"][0];
    assert_eq!(result["status"], "passed", "{body}");
    assert!(result["message"]
        .as_str()
        .unwrap()
        .contains("OK [redacted] [redacted] [redacted]"));
    for forbidden in ["synthetic", "-long-secret", "-long-session"] {
        assert!(!body.to_string().contains(forbidden), "{body}");
    }
    assert_eq!(*upstream.calls.lock().unwrap(), ["iam-secret"]);
    upstream.finish().await;
}
