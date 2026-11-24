//! Existing public/probe cancellation owners retain only already validated supplier facts.
use super::{
    failure_audit_tests::failed, pipeline_fixture::*, probe_tests::options, test_support::*,
};
use axum::http::Method;
use serde_json::{json, Value};
use std::{future::Future, time::Duration};

pub(super) fn assert_phase(row: &crate::db::GatewayRequestAuditView, phase: usize) {
    let facts = &row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"];
    if phase == 0 {
        assert!(
            facts.is_null(),
            "an unfinished submit body proves no task identity"
        );
        assert_ne!(row.response_id, ID);
    } else {
        assert_eq!(row.response_id, ID);
        assert_eq!(
            facts,
            &json!({"id":ID,"model":"uni-1","type":"image",
            "state":if phase == 1 {"queued"} else {"completed"},
            "usage":{"credits_charged":null},"mediaVerified":false})
        );
    }
}

async fn reach(server: &Server, count: usize, call: impl Future<Output = ()>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        tokio::select! {
            _ = call => panic!("held upstream stage must remain pending"),
            _ = async {
                while server.calls().len() < count { tokio::task::yield_now().await; }
            } => {}
        }
    })
    .await
    .expect("expected upstream stage must be reached");
}

fn pending_failed() -> Value {
    let mut value = failed("image", "uni-1", Value::Null);
    value["usage"] = json!({"credits_charged":null});
    value
}

fn scenario(index: usize) -> (Vec<Reply>, Value) {
    let (mut replies, state, credits, verified) = match index {
        0 => (
            vec![
                Reply::json(201, pending_failed()),
                Reply::json(200, pending_failed()),
            ],
            "failed",
            Value::Null,
            false,
        ),
        1 => {
            let mut completed = job("completed");
            completed["usage"] = json!({"credits_charged":null});
            (
                vec![
                    Reply::json(201, completed.clone()),
                    Reply::png(),
                    Reply::json(200, completed),
                ],
                "completed",
                Value::Null,
                true,
            )
        }
        2 => {
            let mut queued = job("queued");
            queued["usage"] = json!({"credits_charged":0});
            (
                vec![
                    Reply::json(201, queued),
                    Reply::json(200, job("processing")),
                    Reply::json(200, job("completed")),
                ],
                "processing",
                json!(0),
                false,
            )
        }
        _ => {
            let mut completed = job("completed");
            completed["usage"] = json!({"credits_charged":1.25});
            (
                vec![Reply::json(201, completed), Reply::png()],
                "completed",
                json!(1.25),
                false,
            )
        }
    };
    replies.last_mut().unwrap().hold = true;
    (
        replies,
        json!({"id":ID,"model":"uni-1","type":"image","state":state,
        "usage":{"credits_charged":credits},"mediaVerified":verified}),
    )
}

async fn assert_cancelled(fixture: &Fixture, server: &Server, expected: Value, count: usize) {
    let row = fixture.audit().await;
    assert_eq!(row.status, "cancelled");
    assert_eq!(row.response_id, ID);
    assert_eq!(row.provider_account_id.as_deref(), Some("luma-pipeline"));
    assert_eq!(row.resolved_model.as_deref(), Some("uni-1"));
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
    assert_eq!(
        row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"],
        expected
    );
    assert!(row.total_tokens.is_none());
    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
    let text = serde_json::to_string(&row).unwrap();
    for secret in [
        KEY,
        "supplier-private",
        "signature=private",
        "合成媒体测试",
        "assets.example.test",
    ] {
        assert!(!text.contains(secret), "audit leaked {secret}");
    }
    let states = fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(crate::db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(states.iter().all(|s| s.failure_count == 0), "{states:?}");
    server.wait_closed().await;
    let calls = server.calls();
    assert_eq!(calls.len(), count);
    assert_eq!(calls.iter().filter(|call| call.method == "POST").count(), 1);
}

#[tokio::test]
async fn luma_agents_cancel_public_retains_settlement_and_earlier_charge_without_replay() {
    Box::pin(async {
        for index in 0..4 {
            let (replies, expected) = scenario(index);
            let count = replies.len();
            let server = Server::start(replies).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            within(&server.origin, async {
                let mut call = Box::pin(fixture.open("images/generations", body(IMAGE)));
                reach(&server, count, async {
                    call.as_mut().await;
                })
                .await;
                assert_eq!(fixture.balance().await.remaining_messages, Some(1));
                drop(call);
                assert_cancelled(&fixture, &server, expected, count).await;
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_agents_cancel_management_retains_settlement_and_earlier_charge_without_replay() {
    Box::pin(async {
        for index in 0..4 {
            let (replies, expected) = scenario(index);
            let count = replies.len();
            let server = Server::start(replies).await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(support::grant_console_secret_access(&fixture.state)).await;
            within(&server.origin, async {
                let mut call = Box::pin(support::send_console_json(
                    &fixture.state,
                    Method::POST,
                    "/v1/internal/gateway/console/providers/luma-pipeline/probe",
                    options("image", IMAGE),
                    Some(&grant),
                    None,
                ));
                reach(&server, count, async {
                    call.as_mut().await;
                })
                .await;
                drop(call);
                assert_cancelled(&fixture, &server, expected, count).await;
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_agents_observation_rejects_invalid_identity_state_and_usage() {
    use crate::protocol::luma_agents::generation_audit::{self, GenerationAudit};
    for field in ["id", "model", "type", "state", "usage"] {
        let mut invalid = job("queued");
        invalid[field] = json!("invalid-private");
        let server = Server::start(vec![Reply::json(201, invalid)]).await;
        let audit = GenerationAudit::default();
        let result = generation_audit::within(
            Some(audit.clone()),
            within(
                &server.origin,
                super::execute(
                    &server.client(),
                    Duration::from_secs(5),
                    &payload(),
                    &request(json!({"prompt":"image"})),
                    "uni-1",
                ),
            ),
        )
        .await;
        assert_eq!(
            result.unwrap_err().code.as_deref(),
            Some("luma_agents_generation")
        );
        assert!(audit.record(&mut json!({})).is_none());
        assert_eq!(server.calls().len(), 1);
    }
}
