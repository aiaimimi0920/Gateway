//! Failed-task evidence stays bounded, task-specific and internal to the audit chain.
use super::{execute, failure_audit_tests::failed, pipeline_fixture::KEY, test_support::*};
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::luma_agents::{record_failure_audit, ADAPTER},
};
use serde_json::{json, Value};
use std::time::Duration;

fn pending() -> Value {
    let mut value = failed("image", "uni-1", Value::Null);
    value["usage"] = json!({"credits_charged":null});
    value
}
async fn run(server: &Server, duration: Duration) -> GatewayError {
    within(
        &server.origin,
        execute(
            &server.client(),
            duration,
            &payload(),
            &request(json!({"prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err()
}
fn receipt(error: &GatewayError, credits: Value) {
    assert_eq!(error.kind, ErrorKind::ContentFilter);
    assert_eq!(error.code.as_deref(), Some("luma_agents_content_moderated"));
    assert!(!error.retryable && !error.should_fallback());
    let mut trace = json!({"existing":true});
    assert_eq!(
        record_failure_audit(&error.clone(), &mut trace).as_deref(),
        Some(ID)
    );
    assert_eq!(
        trace["lumaAgentsGeneration"],
        json!({"id":ID,"model":"uni-1","type":"image",
        "state":"failed","usage":{"credits_charged":credits}})
    );
    assert_eq!(trace["existing"], true);
    assert!(!trace.to_string().contains("supplier-private"));
}
fn wire(server: &Server, count: usize) {
    let calls = server.calls();
    assert_eq!(calls.len(), count);
    assert_eq!(calls.iter().filter(|c| c.method == "POST").count(), 1);
    for call in &calls[1..] {
        assert_eq!(call.method, "GET");
        assert_eq!(call.path, format!("/v1/generations/{ID}"));
    }
}

#[tokio::test]
async fn luma_failure_audit_late_zero_and_fraction_keep_the_original_failure_classification() {
    for credits in [json!(0), json!(2.75)] {
        let mut settled = failed("image", "uni-1", credits.clone());
        settled["failure_code"] = json!("budget_exhausted");
        let server =
            Server::start(vec![Reply::json(201, pending()), Reply::json(200, settled)]).await;
        receipt(&run(&server, Duration::from_secs(5)).await, credits);
        wire(&server, 2);
    }
}

#[tokio::test]
async fn luma_failure_audit_rejects_foreign_task_changed_state_and_bad_charges() {
    for (field, value) in [
        ("id", json!("00000000-0000-0000-0000-000000000001")),
        ("model", json!("uni-1-max")),
        ("type", json!("video")),
        ("state", json!("completed")),
        ("usage", json!({"credits_charged":-1})),
        ("usage", json!({"credits_charged":"1"})),
    ] {
        let mut lookup = failed("image", "uni-1", json!(42));
        lookup[field] = value;
        // An otherwise valid completed reply must still not rewrite the original failed task.
        if field == "state" {
            lookup["failure_code"] = Value::Null;
            lookup["failure_reason"] = Value::Null;
        }
        let server =
            Server::start(vec![Reply::json(201, pending()), Reply::json(200, lookup)]).await;
        receipt(&run(&server, Duration::from_secs(5)).await, Value::Null);
        wire(&server, 2);
    }
}

#[tokio::test]
async fn luma_failure_audit_lookup_http_failure_and_poll_limit_keep_unknown_not_zero() {
    for status in [401, 429, 503] {
        let server = Server::start(vec![
            Reply::json(201, pending()),
            Reply::json(status, json!({"private":KEY})),
        ])
        .await;
        receipt(&run(&server, Duration::from_secs(5)).await, Value::Null);
        wire(&server, 2);
    }
    let server = Server::start(
        std::iter::once(Reply::json(201, pending()))
            .chain((0..3).map(|_| Reply::json(200, pending())))
            .collect(),
    )
    .await;
    receipt(&run(&server, Duration::from_secs(5)).await, Value::Null);
    wire(&server, 4);
}

#[tokio::test]
async fn luma_failure_audit_lookup_deadline_keeps_failure_and_closes_response_body() {
    let server = Server::start(vec![
        Reply::json(201, pending()),
        Reply {
            hold: true,
            ..Reply::json(200, pending())
        },
    ])
    .await;
    receipt(&run(&server, Duration::from_secs(1)).await, Value::Null);
    wire(&server, 2);
    server.wait_closed().await;
}

#[tokio::test]
async fn luma_failure_audit_cancel_during_lookup_never_detaches_work_or_reposts() {
    let server = Server::start(vec![
        Reply::json(201, pending()),
        Reply {
            hold: true,
            ..Reply::json(200, pending())
        },
    ])
    .await;
    let origin = server.origin.clone();
    let client = server.client();
    let task = tokio::spawn(async move {
        within(
            &origin,
            execute(
                &client,
                Duration::from_secs(5),
                &payload(),
                &request(json!({"prompt":"image"})),
                "uni-1",
            ),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.calls().len() < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    server.wait_closed().await;
    wire(&server, 2);
}

#[tokio::test]
async fn luma_failure_audit_invalid_initial_receipt_does_not_create_evidence() {
    for (field, value) in [
        ("id", json!("not-a-uuid")),
        ("model", json!("wrong-model")),
        ("type", json!("video")),
        ("state", json!("unexpected")),
        ("usage", json!({"credits_charged":-1})),
    ] {
        let mut invalid = failed("image", "uni-1", json!(1));
        invalid[field] = value;
        let server = Server::start(vec![Reply::json(201, invalid)]).await;
        let error = run(&server, Duration::from_secs(5)).await;
        assert!(error.luma_generation_failure.is_none());
        let mut trace = json!({});
        assert!(record_failure_audit(&error, &mut trace).is_none());
        wire(&server, 1);
    }
}

#[tokio::test]
async fn luma_failure_audit_receipt_is_not_public_or_accepted_under_another_provider() {
    use axum::response::IntoResponse;
    let server = Server::start(vec![Reply::json(
        201,
        failed("image", "uni-1", json!(0.75)),
    )])
    .await;
    let error = run(&server, Duration::from_secs(5)).await;
    receipt(&error, json!(0.75));
    let response = error.clone().into_response();
    let body = super::pipeline_fixture::drain(response.into_body()).await;
    for private in [ID, "credits_charged", "supplier-private", KEY] {
        assert!(!body.contains(private), "{body}");
    }
    let mut trace = json!({});
    assert!(record_failure_audit(&error.with_provider("openai_compatible"), &mut trace).is_none());
    assert!(record_failure_audit(
        &GatewayError::server_error("pretend receipt").with_provider(ADAPTER),
        &mut trace
    )
    .is_none());
    assert_eq!(trace, json!({}));
}
