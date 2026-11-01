//! Network recovery uses the real send gate even for tool requests with zero normal retries.
use super::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

async fn connect_error() -> GatewayError {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    crate::http_client::send_native_codex(
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(format!("http://{address}/")),
        "fixture",
    )
    .await
    .unwrap_err()
}

pub(super) fn tool_gate() -> ProviderAttemptGate {
    let mut gate = tests::gate();
    let req = crate::protocol::responses::normalize_responses(serde_json::json!({
        "input":"hello", "tools":[{"type":"function","name":"fixture","parameters":{"type":"object"}}],
    })).unwrap();
    assert!(!crate::pipeline::request_budget::replay_safe(&req));
    gate.request_budget = crate::pipeline::request_budget::RequestBudget::for_request(&req);
    gate
}

pub(super) fn policy() -> RetryPolicy {
    RetryPolicy {
        max_retries: 0,
        initial_delay: Duration::from_millis(1),
        ..Default::default()
    }
}

#[tokio::test]
async fn safe_connection_failure_recovers_tools_without_penalizing_the_account() {
    let error = connect_error().await;
    let gate = tool_gate();
    let calls = AtomicUsize::new(0);
    gate.execute_observed(
        || async {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(error.clone())
            } else {
                Ok(())
            }
        },
        |_| {},
        &policy(),
    )
    .await
    .unwrap();
    assert_eq!(gate.request_budget.attempts(), 2);
    assert_eq!(gate.outbound_attempt_count.load(Ordering::Relaxed), 2);
    assert_eq!(
        gate.smart_routing
            .score("diagnostic-provider", "fixture-model"),
        100.0
    );
    assert_eq!(gate.request_budget.remaining_attempts(), 0);
}

#[tokio::test]
async fn persistent_fault_has_five_actual_attempts_and_does_not_poison_next_request() {
    let error = connect_error().await;
    let gate = tool_gate();
    let result: Result<(), _> = gate
        .execute_observed(|| async { Err(error.clone()) }, |_| {}, &policy())
        .await;
    assert_eq!(result.unwrap_err().code, error.code);
    assert_eq!(gate.request_budget.attempts(), 5);
    assert_eq!(tool_gate().request_budget.remaining_attempts(), 1);
    assert_eq!(
        gate.smart_routing
            .score("diagnostic-provider", "fixture-model"),
        100.0
    );
}

#[tokio::test]
async fn unknown_delivery_or_forged_provider_code_never_bypasses_tool_replay_guard() {
    let gate = tool_gate();
    let error = GatewayError::service_unavailable("error sending request")
        .with_code("native_transport_connection_refused");
    let result: Result<(), _> = gate
        .execute_observed(|| async { Err(error.clone()) }, |_| {}, &policy())
        .await;
    assert!(result.is_err());
    assert_eq!(gate.request_budget.attempts(), 1);
}

#[tokio::test]
async fn dropping_recovery_during_sleep_stops_all_sends_and_keeps_attempt_accounting() {
    let error = connect_error().await;
    let gate = Arc::new(tool_gate());
    let calls = Arc::new(AtomicUsize::new(0));
    let task = {
        let gate = gate.clone();
        let calls = calls.clone();
        tokio::spawn(async move {
            let policy = RetryPolicy {
                max_retries: 0,
                initial_delay: Duration::from_secs(5),
                ..Default::default()
            };
            let _: Result<(), _> = gate
                .execute_observed(
                    || async {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Err(error.clone())
                    },
                    |_| {},
                    &policy,
                )
                .await;
        })
    };
    while calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    task.abort();
    let _ = task.await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(gate.request_budget.attempts(), 1);
}

#[tokio::test]
async fn recovered_connection_does_not_apply_a_thirty_second_model_response_timeout() {
    let error = connect_error().await;
    tokio::time::pause();
    let gate = tool_gate();
    let calls = AtomicUsize::new(0);
    gate.request_budget
        .run(gate.execute_observed(
            || async {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err(error.clone());
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(())
            },
            |_| {},
            &policy(),
        ))
        .await
        .unwrap();
    assert_eq!(gate.request_budget.attempts(), 2);
}

#[tokio::test]
async fn original_deadline_cancels_recovery_and_cannot_be_extended_by_connect_allowance() {
    let error = connect_error().await;
    tokio::time::pause();
    let mut gate = tool_gate();
    gate.request_budget =
        crate::pipeline::request_budget::RequestBudget::new(1, Duration::from_secs(1));
    let policy = RetryPolicy {
        max_retries: 0,
        initial_delay: Duration::from_secs(5),
        ..Default::default()
    };
    let result: Result<(), _> = gate
        .request_budget
        .run(gate.execute_observed(|| async { Err(error.clone()) }, |_| {}, &policy))
        .await;
    assert_eq!(
        result.unwrap_err().request_budget_stop_reason(),
        Some("deadline")
    );
    assert_eq!(gate.request_budget.attempts(), 1);
}
