//! Run alone: cancellation intentionally quarantines the process-wide SDK text slot.
use super::*;
use std::{path::Path, time::Duration};

fn sent_calls(root: &Path) -> usize {
    std::fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with("-rpc.jsonl"))
        .map(|entry| {
            std::fs::read_to_string(entry.path())
                .unwrap()
                .matches("session.sendAndWait")
                .count()
        })
        .sum()
}

fn terminal(result: Result<PipelineOutput, AttemptError>) -> GatewayError {
    match result {
        Err(AttemptError::Stop(error)) => error,
        Err(AttemptError::Next(error)) => panic!("SDK must not select another candidate: {error}"),
        Ok(_) => panic!("expected terminal failure"),
    }
}

async fn failed_audit(f: &fixture::Fixture, error: &GatewayError, expected: &str) {
    assert!(!error.retryable);
    assert!(error.native_transport.is_none());
    assert!(!crate::request_wait::recoverable(error));
    assert!(!format!("{error:?}").contains("synthetic-token"));
    assert_eq!(f.ctx.observed_upstream_status, Some(None));
    assert_eq!(f.ctx.route_attempt_count(), 1);
    super::super::super::stage_finalize::run_failure(error, &f.ctx, &f.console.state).await;
    let rows = f
        .local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, expected);
    assert!(rows[0].upstream_status.is_none());
    assert!(rows[0].prompt_tokens.is_none());
    assert!(rows[0].completion_tokens.is_none());
    assert!(rows[0].total_tokens.is_none());
    assert_eq!(
        rows[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
        1
    );
    assert!(!serde_json::to_string(&rows)
        .unwrap()
        .contains("synthetic-token"));
}

async fn request(prompt: &str) -> fixture::Fixture {
    let mut f = configured().await;
    f.ctx.canonical_req = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"public-alias", "messages":[{"role":"user","content":prompt}]
    }))
    .unwrap();
    super::super::super::stage_finalize::begin_request_audit(&mut f.ctx, &f.console.state).await;
    f
}

#[tokio::test]
#[ignore = "run alone with synthetic CLI and fresh probe/workspace roots; quarantines global text slot"]
async fn copilot_sender_termination_denial_then_caller_cancel_never_replays() {
    let root = std::path::PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap());
    assert_eq!(sent_calls(&root), 0, "requires fresh evidence directory");
    let mut f = request("denied").await;
    let before = balance_snapshot(&f).await;
    let mut attempt = prepared(&f).await;
    // Caller retry preferences cannot override the SDK's no-replay contract.
    attempt.retry_policy.max_retries = 3;
    let error = terminal(
        recovery_wait::send_cancellable(&mut f.ctx, &f.console.state, &f.candidate, 0, attempt)
            .await,
    );
    assert_eq!(error.code.as_deref(), Some("copilot_execution_unavailable"));
    assert_eq!(
        axum::response::IntoResponse::into_response(error.clone()).status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    failed_audit(&f, &error, "failed").await;
    assert_eq!(balance_snapshot(&f).await, before);
    assert_eq!(sent_calls(&root), 1);
    finish(f).await;

    // A confirmed SDK failure released the slot: the next request must reach generation.
    let mut f = request("hang").await;
    let before = balance_snapshot(&f).await;
    let (cancel, receiver) = tokio::sync::watch::channel(false);
    f.ctx.caller_cancel = Some(receiver);
    let attempt = prepared(&f).await;
    let send =
        recovery_wait::send_cancellable(&mut f.ctx, &f.console.state, &f.candidate, 0, attempt);
    let cancel_after_send = async {
        loop {
            if sent_calls(&root) == 2 {
                cancel.send(true).unwrap();
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(30), async {
        tokio::join!(send, cancel_after_send)
    })
    .await
    .expect("caller cancellation must finish without the SDK's generation timeout");
    let error = terminal(result);
    assert_eq!(
        f.ctx.request_budget.stopped_reason(),
        Some("caller_cancelled")
    );
    failed_audit(&f, &error, "cancelled").await;
    assert_eq!(balance_snapshot(&f).await, before);
    assert_eq!(sent_calls(&root), 2);
    finish(f).await;

    // An unconfirmed Drop must not start another child even after the cancelled request returned.
    let mut f = request("ok").await;
    let attempt = prepared(&f).await;
    let error = terminal(
        recovery_wait::send_cancellable(&mut f.ctx, &f.console.state, &f.candidate, 0, attempt)
            .await,
    );
    assert_eq!(error.code.as_deref(), Some("copilot_execution_unavailable"));
    assert_eq!(sent_calls(&root), 2);
    let workspaces = std::env::var_os("GATEWAY_COPILOT_WORKSPACE_ROOT").unwrap();
    assert_eq!(std::fs::read_dir(workspaces).unwrap().count(), 1);
    finish(f).await;
}

#[tokio::test]
async fn copilot_sender_termination_failure_audit_preserves_http_compatibility() {
    for (observed, expected) in [
        (None, Some(503)),
        (Some(None), None),
        (Some(Some(502)), Some(502)),
    ] {
        let mut f = request("ok").await;
        f.ctx
            .route_attempt_count
            .store(1, std::sync::atomic::Ordering::Relaxed);
        f.ctx.observed_upstream_status = observed;
        let error = GatewayError::service_unavailable("synthetic failure");
        super::super::super::stage_finalize::run_failure(&error, &f.ctx, &f.console.state).await;
        let rows = f
            .local
            .list_audits(&crate::db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].upstream_status, expected);
        assert_eq!(rows[0].status, "failed");
        finish(f).await;
    }
}
