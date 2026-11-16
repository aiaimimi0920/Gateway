//! Full Local pipeline, real management/TCP supplier; no generation task or synthetic cost on cancel.
use super::*;
use crate::pipeline::{request_budget::RequestBudget, PipelineContext, PipelineOutput};
use std::sync::Arc;
use tokio::sync::{watch, Notify};
mod edges;
mod journal;
mod phases;

pub(super) fn stream(mode: &str) -> Option<axum::response::Response> {
    use axum::{body::Body, http::header::CONTENT_TYPE, response::IntoResponse};
    use futures::StreamExt;
    if !mode.starts_with("stream_") {
        return None;
    }
    let mut value = json!({"id":"native-id","model":"fbm1.open","choices":[{"index":0,
        "delta":{"role":"assistant","content":"partial"},"finish_reason":"stop"}]});
    if mode == "stream_known" {
        value["usage"] = json!({"cost":0.125});
    }
    let frame = format!("data: {value}\n\n");
    let body = futures::stream::once(async move {
        Ok::<_, std::convert::Infallible>(bytes::Bytes::from(frame))
    })
    .chain(futures::stream::pending());
    Some(
        (
            [(CONTENT_TYPE, "text/event-stream")],
            Body::from_stream(body),
        )
            .into_response(),
    )
}

fn caller(
    host: &Host,
    peer: &peer::Peer,
    cause: &str,
    observed: Arc<Notify>,
) -> (
    watch::Sender<bool>,
    tokio::task::JoinHandle<Result<PipelineOutput, crate::error::GatewayError>>,
) {
    let req = crate::protocol::openai::normalize_chat_completions(request()).unwrap();
    let mut ctx = PipelineContext::new(req, Some(host.token.clone()));
    ctx.request_budget = RequestBudget::new(
        6,
        Duration::from_secs(if cause == "deadline" { 2 } else { 10 }),
    );
    let (cancel, receiver) = watch::channel(false);
    let state = host.state.clone();
    let origin = peer.server.origin.clone();
    let task = tokio::spawn(async move {
        crate::protocol::freebuff::catalog::with_test_origin(
            origin,
            crate::protocol::freebuff::catalog::TEST_BODY_STARTED.scope(
                observed,
                crate::pipeline::run_pipeline_cancellable(ctx, &state, receiver),
            ),
        )
        .await
    });
    (cancel, task)
}
fn signal(host: &Host, cause: &str, cancel: watch::Sender<bool>) -> Option<watch::Sender<bool>> {
    match cause {
        "caller" => {
            cancel.send(true).unwrap();
        }
        "closed" => return None,
        "shutdown" => {
            host.state.shutdown.request("synthetic_paid_cancel");
        }
        "deadline" => (),
        _ => panic!("unknown cause"),
    }
    Some(cancel)
}
async fn body_ready(pause: &peer::Pause, observed: &Notify) {
    use futures::FutureExt;
    pause.reached().await;
    // Session-wire notifications are earlier phases, not proof of parsed model cost.
    while observed.notified().now_or_never().is_some() {}
    pause.resume();
    tokio::time::timeout(Duration::from_secs(3), observed.notified())
        .await
        .unwrap();
}
async fn approve(
    host: &Host,
    task: &tokio::task::JoinHandle<Result<PipelineOutput, crate::error::GatewayError>>,
) {
    let grant = host.grant().await;
    let view = host.pending(&grant, task).await;
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
}
async fn result(
    task: tokio::task::JoinHandle<Result<PipelineOutput, crate::error::GatewayError>>,
) -> crate::error::GatewayError {
    match tokio::time::timeout(Duration::from_secs(4), task)
        .await
        .expect("cooperative closeout must complete")
        .unwrap()
    {
        Err(error) => error,
        Ok(_) => panic!("cancelled public generation must not succeed"),
    }
}
async fn invariant(host: &Host, peer: &peer::Peer, model: usize, cause: &str) {
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
    assert_eq!(peer.calls("/api/v1/chat/completions"), model);
    assert_eq!(peer.calls("/api/v1/freebuff/session/attempt"), 0);
    assert_eq!(claims(host).await, 1, "FINISH is not end");
    let status = if matches!(cause, "deadline" | "upstream") {
        "failed"
    } else {
        "cancelled"
    };
    settled(host, status, 10).await;
    assert_eq!(send_fences::attempts(host, 1).await, [1]);
    let rows = host
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    let row = &rows[0];
    assert_eq!(
        row.client_disconnected_at.is_some(),
        matches!(cause, "caller" | "closed")
    );
    if cause != "upstream" {
        let trace = row.route_trace.as_ref().unwrap();
        assert_eq!(
            trace["terminationReason"],
            match cause {
                "caller" | "closed" => "caller_cancelled",
                other => other,
            }
        );
        assert!(
            trace["failureClass"].is_null(),
            "host cancellation is not credential failure"
        );
    }
}
#[tokio::test]
async fn freebuff_public_paid_cancel_after_report_finishes_original_run_without_zero_or_end() {
    for cause in ["caller", "closed", "shutdown", "deadline"] {
        let peer = peer::Peer::new().await;
        let pause = peer.pause("/api/v1/chat/completions", 1);
        *peer.mode.lock().unwrap() = "stream_known".into();
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let observed = Arc::new(Notify::new());
        let (cancel, task) = caller(&host, &peer, cause, observed.clone());
        approve(&host, &task).await;
        body_ready(&pause, &observed).await;
        let _keep = signal(&host, cause, cancel);
        let error = result(task).await;
        assert_eq!(
            error.request_budget_stop_reason(),
            Some(match cause {
                "caller" | "closed" => "caller_cancelled",
                other => other,
            })
        );
        let finishes = peer.finishes();
        assert_eq!(
            finishes.len(),
            1,
            "{cause}: known reported cost needs explicit FINISH"
        );
        assert_eq!(finishes[0]["runId"], "synthetic-run");
        assert_eq!(finishes[0]["status"], "cancelled");
        assert_eq!(finishes[0]["totalSteps"], 1);
        assert_eq!(finishes[0]["directCredits"], 13);
        assert_eq!(finishes[0]["totalCredits"], 13);
        invariant(&host, &peer, 1, cause).await;
        host.close().await;
        peer.server.close().await;
    }
}
