//! Real periodic worker and durable schedule slots for the Nous Subscription and Welcome probe targets.
use super::{start_provider_credential_probe_scheduler, sweep_scheduled_provider_credentials_once};
use crate::{db, protocol::nous_portal, provider_runtime::test_store, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};
mod fixture;
#[path = "../protocol/dim_oauth/pipeline_tests/peer.rs"]
mod peer;
#[path = "../../tests/console_contract_support/mod.rs"]
mod support;
use fixture::{document, native, Fixture};

#[test]
fn nous_scheduler_real_ticks_persist_independent_plans_and_restart_slots() {
    fixture::run(real_ticks);
}
async fn real_ticks() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let root = std::env::temp_dir().join(format!("product-schedule-{}", uuid::Uuid::new_v4()));
        let f = Fixture::new(&peer, &root, document(true)).await;
        let state = f.state().clone();
        // The worker runs unmodified: no paused clock, fake sweep, or edited next-due timestamp.
        let worker = Box::pin(with_origin(
            &peer.origin,
            start_provider_credential_probe_scheduler(state),
        ));
        let observe = async {
            f.wait_audits(4).await;
            assert_eq!(peer.calls.lock().unwrap().len(), 4);
            assert_results(f.state(), "passed").await;
            let skipped = with_origin(
                &peer.origin,
                sweep_scheduled_provider_credentials_once(f.state()),
            )
            .await
            .unwrap();
            assert_eq!(skipped.due_count, 0);
            assert_eq!(skipped.skipped_count, 6);
            // Wait for the real 60-second timer and wall-clock slot expiry; allow a boundary tick.
            f.wait_audits(8).await;
            f.state().shutdown.request("scheduler-contract-finished");
        };
        tokio::time::timeout(Duration::from_secs(135), async {
            tokio::join!(worker, observe);
        })
        .await
        .expect("periodic worker did not execute a due round or stop");
        assert_eq!(peer.calls.lock().unwrap().len(), 8);
        f.assert_audits(8, "completed", Some(5)).await;
        assert_wire(&peer, 8);
        f.close().await;
        let restarted = Fixture::new(&peer, &root, document(true)).await;
        assert_results(restarted.state(), "passed").await;
        let summary = with_origin(
            &peer.origin,
            sweep_scheduled_provider_credentials_once(restarted.state()),
        )
        .await
        .unwrap();
        assert_eq!(
            summary.due_count, 0,
            "restart must not reset persisted interval slots"
        );
        assert_eq!(peer.calls.lock().unwrap().len(), 8);
        restarted.close().await;
        peer.stop().await;
    })
    .await;
}

#[test]
fn nous_scheduler_failure_is_saved_without_replay_or_false_usage() {
    fixture::run(failure);
}
async fn failure() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 401).await;
        let root =
            std::env::temp_dir().join(format!("product-schedule-failure-{}", uuid::Uuid::new_v4()));
        let f = Fixture::new(&peer, &root, document(true)).await;
        let summary = with_origin(
            &peer.origin,
            sweep_scheduled_provider_credentials_once(f.state()),
        )
        .await
        .unwrap();
        assert_eq!(
            (
                summary.scheduled_count,
                summary.due_count,
                summary.failed_count
            ),
            (6, 4, 4)
        );
        assert_eq!(summary.passed_count, 0);
        assert_results(f.state(), "failed").await;
        f.assert_audits(4, "failed", None).await;
        let again = with_origin(
            &peer.origin,
            sweep_scheduled_provider_credentials_once(f.state()),
        )
        .await
        .unwrap();
        assert_eq!(again.due_count, 0);
        assert_wire(&peer, 4);
        f.close().await;
        peer.stop().await;
    })
    .await;
}

#[test]
fn nous_scheduler_disabled_policies_and_shutdown_never_generate() {
    fixture::run(disabled);
}
async fn disabled() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let root =
            std::env::temp_dir().join(format!("product-schedule-off-{}", uuid::Uuid::new_v4()));
        let f = Fixture::new(&peer, &root, document(false)).await;
        let summary = sweep_scheduled_provider_credentials_once(f.state())
            .await
            .unwrap();
        assert_eq!(summary.scheduled_count, 0);
        f.close().await;
        let f = Fixture::new(&peer, &root, document(true)).await;
        f.state().shutdown.request("already-stopped");
        tokio::time::timeout(
            Duration::from_secs(2),
            start_provider_credential_probe_scheduler(f.state().clone()),
        )
        .await
        .unwrap();
        assert!(peer.calls.lock().unwrap().is_empty());
        f.assert_audits(0, "completed", None).await;
        f.close().await;
        peer.stop().await;
    })
    .await;
}

async fn with_origin<T>(origin: &str, task: impl std::future::Future<Output = T>) -> T {
    nous_portal::test_support::origin(origin.into(), task).await
}

async fn assert_results(state: &AppState, status: &str) {
    for provider in ["subscription", "welcome"] {
        let active = format!("{provider}-active");
        let disabled = format!("{provider}-disabled");
        let off = format!("{provider}-off");
        for plan in [None, Some("named")] {
            let results = test_store::read_many(state, provider, &[active.as_str()], plan)
                .await
                .unwrap();
            assert_eq!(results.len(), 1);
            let raw = serde_json::to_value(&results[0]).unwrap();
            assert_eq!(raw["status"], status);
            let assessment = results[0].assessment.as_ref().unwrap();
            assert_eq!(assessment.mode, "automatic");
            assert_eq!(assessment.plan_id.as_deref(), plan);
            assert_eq!(
                assessment.policy_source,
                if plan.is_some() {
                    "plan:named"
                } else {
                    "account"
                }
            );
            assert_eq!(assessment.cases.len(), 1);
            assert_eq!(
                assessment.cases[0].correct,
                if status == "passed" { Some(true) } else { None }
            );
            for secret in [
                fixture::access(&active),
                format!("nous-refresh-{active}"),
                "synthetic-private-error".into(),
            ] {
                assert!(!raw.to_string().contains(&secret));
            }
        }
        assert!(
            test_store::read_many(state, provider, &[disabled.as_str(), off.as_str()], None)
                .await
                .unwrap()
                .is_empty()
        );
    }
}

fn assert_wire(peer: &peer::Peer, count: usize) {
    let calls = peer.calls.lock().unwrap();
    assert_eq!(calls.len(), count);
    for (path, headers, body) in calls.iter() {
        assert_eq!(path, "/v1/chat/completions");
        assert!(["subscription-active", "welcome-active"]
            .iter()
            .any(|id| headers["authorization"] == format!("Bearer {}", fixture::access(id))));
        assert_eq!(headers["user-agent"], "NeuroGateway/NousPortal");
        assert_eq!(body["model"], "org/model");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["content"], "only OK");
        assert!(!headers.contains_key("cookie"));
    }
}
