//! Stage-error outputs and SSE handoff keep a single real SQLite quota finalizer.
use super::{test_support::*, Owner, Semaphore, Stage};
use crate::{
    pipeline::{stage_finalize, PipelineOutput},
    upstream::stream::TrackedStream,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

#[test]
fn pipeline_lifecycle_route_error_claimed_or_unclaimed_keeps_one_refund() {
    run(route_error);
}
async fn route_error() {
    for claimed in [false, true] {
        let fixture = Fixture::new().await;
        let limit = Arc::new(Semaphore::new(1));
        let mut owner = Owner::begin(fixture.ctx(), &fixture.console.state, limit.clone())
            .await
            .unwrap();
        for stage in [Stage::Audit, Stage::Filter] {
            let (next, result) = owner.step(stage).await.unwrap();
            owner = next;
            result.unwrap();
        }
        assert!(fixture.balance().await.0 < 1000);
        if claimed {
            let (owner, result) = owner.step(Stage::Route).await.unwrap();
            let error = result.expect_err("Empty route fixture must fail before sending");
            assert!(owner.finish(Err(error)).await.is_err());
        } else {
            let pause = Arc::new(Pause::default());
            let returning = Arc::new(tokio::sync::Notify::new());
            owner.hooks.after = Some((Stage::Route, pause.clone()));
            owner.hooks.returning = Some(returning.clone());
            let mut pending: PinFuture<'_> = Box::pin(async {
                let _ = owner.step(Stage::Route).await;
                panic!("Unclaimed route error reached caller");
            });
            ready(&pause, &mut pending).await;
            pause.release();
            returning.notified().await;
            drop(pending);
        }
        let returned = tokio::time::timeout(Duration::from_secs(3), limit.acquire_owned())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fixture.balance().await, (1000, 0));
        let row = fixture
            .audit(if claimed { "failed" } else { "cancelled" })
            .await;
        assert!(row.total_tokens.is_none() && row.client_disconnected_at.is_none());
        assert_eq!(
            row.route_trace.as_ref().unwrap()["routeAttemptCount"].as_u64(),
            Some(0)
        );
        drop(returned);
        fixture.close().await;
    }
}

#[test]
fn pipeline_lifecycle_sse_handoff_then_immediate_drop_refunds_only_in_stream() {
    run(sse_handoff);
}
async fn sse_handoff() {
    let fixture = Fixture::new().await;
    let limit = Arc::new(Semaphore::new(1));
    let mut owner = Owner::begin(fixture.ctx(), &fixture.console.state, limit.clone())
        .await
        .unwrap();
    for stage in [Stage::Audit, Stage::Filter] {
        let (next, result) = owner.step(stage).await.unwrap();
        owner = next;
        result.unwrap();
    }
    let reserved = fixture.balance().await;
    assert!(reserved.0 < 1000);
    let ctx = owner.ctx.as_ref().unwrap();
    let audit = stage_finalize::snapshot_request_audit(ctx);
    let snapshot = stage_finalize::FailureFinalizeSnapshot {
        request_id: ctx.req_id.to_string(),
        credential_id: ctx.quota_credential_id.clone(),
        project_id: audit.project_id.clone(),
        user_id: audit.user_id.clone(),
        model: audit.resolved_model.clone(),
        provider: audit.selected_adapter.clone(),
        started_at: ctx.started_at,
        pre_deducted_tokens: ctx.quota_pre_deducted_tokens,
        request_audit: audit,
    };
    let state = fixture.console.state.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    // This unit callback uses the production stream finalizer; public-router stream tests cover packing.
    let stream = TrackedStream::new_with_error(futures::stream::pending(), move |_, success| {
        assert!(!success);
        observed.fetch_add(1, Ordering::SeqCst);
        let drain = state.local_runtime.as_ref().unwrap().track_finalizer();
        tokio::spawn(async move {
            stage_finalize::run_stream_failure(
                snapshot,
                &state,
                "Synthetic consumer closed",
                stage_finalize::StreamFailureCause::ConsumerClosed,
            )
            .await;
            drop(drain);
        });
    });
    let result = owner.finish(Ok(PipelineOutput::Sse(stream))).await.unwrap();
    assert_eq!(
        limit.available_permits(),
        1,
        "SSE must release the lifecycle slot synchronously"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        fixture.balance().await,
        reserved,
        "Lifecycle must not refund the transferred stream"
    );
    drop(result);
    let row = fixture.audit("cancelled").await;
    assert!(row.total_tokens.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.balance().await, (1000, 0));
    fixture.close().await;
}
