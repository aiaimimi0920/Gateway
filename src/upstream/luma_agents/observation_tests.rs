//! Observation scopes isolate concurrent requests and publish settled charges before returning.
use super::{failure_audit_tests::failed, test_support::*};
use crate::protocol::luma_agents::generation_audit::{self, GenerationAudit};
use serde_json::{json, Value};
use std::time::Duration;

async fn run(
    server: &Server,
    audit: &GenerationAudit,
) -> Result<Value, crate::error::GatewayError> {
    generation_audit::within(
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
    .await
}

#[tokio::test]
async fn luma_agents_observation_publishes_late_credits_before_terminal_handoff() {
    for failure in [false, true] {
        let mut terminal = if failure {
            failed("image", "uni-1", Value::Null)
        } else {
            job("completed")
        };
        terminal["usage"] = json!({"credits_charged":null});
        let mut settled = terminal.clone();
        settled["usage"]["credits_charged"] = json!(0.5);
        let mut replies = vec![Reply::json(201, terminal)];
        if !failure {
            replies.push(Reply::png());
        }
        replies.push(Reply::json(200, settled));
        let server = Server::start(replies).await;
        let audit = GenerationAudit::default();
        assert_eq!(run(&server, &audit).await.is_err(), failure);
        let mut trace = json!({});
        assert_eq!(audit.record(&mut trace).as_deref(), Some(ID));
        assert_eq!(
            trace["lumaAgentsGeneration"]["usage"]["credits_charged"],
            0.5
        );
        assert_eq!(trace["lumaAgentsGeneration"]["mediaVerified"], !failure);
    }
}

#[tokio::test]
async fn luma_agents_observation_rejects_mismatched_late_settlement() {
    let mut terminal = failed("image", "uni-1", Value::Null);
    terminal["usage"] = json!({"credits_charged":null});
    let mut wrong = terminal.clone();
    wrong["model"] = json!("uni-1-max");
    wrong["usage"]["credits_charged"] = json!(5);
    let server = Server::start(vec![Reply::json(201, terminal), Reply::json(200, wrong)]).await;
    let audit = GenerationAudit::default();
    assert!(run(&server, &audit).await.is_err());
    let mut trace = json!({});
    audit.record(&mut trace).unwrap();
    assert_eq!(trace["lumaAgentsGeneration"]["state"], "failed");
    assert!(trace["lumaAgentsGeneration"]["usage"]["credits_charged"].is_null());
}

#[tokio::test]
async fn luma_agents_observation_concurrent_scopes_do_not_exchange_task_facts() {
    let ids = [ID, "f47095e6-7c32-4917-b65c-3462f65c0f6c"];
    let mut servers = Vec::new();
    for (index, id) in ids.iter().enumerate() {
        let mut completed = job("completed");
        completed["id"] = json!(id);
        completed["usage"] = json!({"credits_charged":index});
        servers.push(
            Server::start(vec![
                Reply::json(201, completed),
                Reply {
                    hold: true,
                    ..Reply::png()
                },
            ])
            .await,
        );
    }
    let audits = [GenerationAudit::default(), GenerationAudit::default()];
    let mut first = Box::pin(run(&servers[0], &audits[0]));
    let mut second = Box::pin(run(&servers[1], &audits[1]));
    tokio::time::timeout(Duration::from_secs(3), async {
        tokio::select! {
            _ = first.as_mut() => panic!("first asset is held"),
            _ = second.as_mut() => panic!("second asset is held"),
            _ = async {
                while servers.iter().any(|s| s.calls().len() < 2) { tokio::task::yield_now().await; }
            } => {}
        }
    }).await.unwrap();
    drop(first);
    drop(second);
    for (index, audit) in audits.iter().enumerate() {
        let mut trace = json!({});
        assert_eq!(audit.record(&mut trace).as_deref(), Some(ids[index]));
        assert_eq!(
            trace["lumaAgentsGeneration"]["usage"]["credits_charged"],
            index
        );
        servers[index].wait_closed().await;
        assert_eq!(servers[index].calls().len(), 2);
    }
    // A new unscoped sender cannot mutate either request's retained snapshot.
    let mut fresh = job("failed");
    fresh["usage"] = json!({"credits_charged":99});
    let server = Server::start(vec![Reply::json(201, fresh)]).await;
    assert!(within(
        &server.origin,
        super::execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &request(json!({"prompt":"image"})),
            "uni-1"
        )
    )
    .await
    .is_err());
    let mut trace = json!({});
    audits[0].record(&mut trace).unwrap();
    assert_eq!(trace["lumaAgentsGeneration"]["usage"]["credits_charged"], 0);
}
