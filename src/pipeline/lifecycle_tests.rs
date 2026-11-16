use super::{test_support::*, Owner, Semaphore, Stage};
use crate::{error::GatewayError, pipeline::PipelineOutput};
use futures::FutureExt;
use serde_json::json;
use std::{sync::Arc, time::Duration};

#[test]
fn pipeline_lifecycle_admission_unclaimed_outputs_cancel_and_refund_once() {
    run(admission_drop);
}
async fn admission_drop() {
    for target in [Stage::Audit, Stage::Filter] {
        for phase in [0, 1, 2] {
            let fixture = Fixture::new().await;
            let pause = Arc::new(Pause::default());
            let returning = Arc::new(tokio::sync::Notify::new());
            let mut hooks = Hooks::default();
            if phase == 0 {
                hooks.before = Some((target, pause.clone()));
            } else {
                hooks.after = Some((target, pause.clone()));
                hooks.returning = Some(returning.clone());
            }
            let limit = Arc::new(Semaphore::new(1));
            let mut owner = hooks
                .within(Owner::begin(
                    fixture.ctx(),
                    &fixture.console.state,
                    limit.clone(),
                ))
                .await
                .unwrap();
            if target == Stage::Filter {
                let (next, result) = owner.step(Stage::Audit).await.unwrap();
                owner = next;
                result.unwrap();
                // The previous stage's completion signal must not satisfy the target-stage barrier.
                let _ = returning.notified().now_or_never();
            }
            let mut pending: PinFuture<'_> = Box::pin(async {
                let _ = owner.step(target).await;
                panic!("Unclaimed stage reached caller");
            });
            ready(&pause, &mut pending).await;
            assert_eq!(limit.available_permits(), 0);
            if target == Stage::Filter && phase > 0 {
                let (balance, inflight) = fixture.balance().await;
                assert!(balance < 1000);
                assert_eq!(inflight, 1);
            }
            if phase == 2 {
                pause.release();
                returning.notified().await;
                // Same current-thread executor, no await between returning signal and owned output.
                assert_eq!(limit.available_permits(), 0);
            }
            drop(pending);
            if phase != 2 {
                pause.release();
            }
            let row = fixture.audit("cancelled").await;
            assert!(row.total_tokens.is_none() && row.client_disconnected_at.is_none());
            let returned = tokio::time::timeout(Duration::from_secs(3), limit.acquire_owned())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(fixture.balance().await, (1000, 0));
            assert_eq!(
                hooks.events.lock().unwrap().len(),
                if target == Stage::Filter { 2 } else { 1 }
            );
            drop(returned);
            fixture.close().await;
        }
    }
}
#[test]
fn pipeline_lifecycle_submitted_success_and_failure_survive_waiter_drop_without_double_refund() {
    run(terminal_drop);
}
async fn terminal_drop() {
    for success in [true, false] {
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
        let ctx = owner.ctx.as_mut().unwrap();
        ctx.selected_provider_id = Some("synthetic-provider".into());
        ctx.selected_real_credential_ref = Some("synthetic-exact".into());
        ctx.resolved_model = Some("synthetic-model".into());
        ctx.selected_adapter = Some("openai_compatible".into());
        let result = if success {
            Ok(PipelineOutput::Json(
                json!({"usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10}}),
            ))
        } else {
            Err(GatewayError::bad_request("Expected synthetic failure"))
        };
        let pause = Arc::new(Pause::default());
        owner.hooks.terminal = Some(pause.clone());
        let mut pending: PinFuture<'_> = Box::pin(async {
            let _ = owner.finish(result).await;
        });
        ready(&pause, &mut pending).await;
        let transaction = fixture
            .local
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .unwrap();
        drop(pending);
        pause.release();
        let mut closing = Box::pin(fixture.local.close());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), closing.as_mut())
                .await
                .is_err()
        );
        assert_eq!(limit.available_permits(), 0);
        transaction.rollback().await.unwrap();
        let returned = tokio::time::timeout(Duration::from_secs(3), limit.acquire_owned())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            fixture.balance().await,
            (if success { 990 } else { 1000 }, 0)
        );
        let row = fixture
            .audit(if success { "completed" } else { "failed" })
            .await;
        assert_eq!(row.total_tokens, success.then_some(10));
        drop(returned);
        tokio::time::timeout(Duration::from_secs(3), closing)
            .await
            .unwrap();
        drop(fixture);
    }
}
#[test]
fn pipeline_lifecycle_queued_admission_obeys_caller_deadline_and_shutdown_without_storage() {
    run(queued);
}
async fn queued() {
    for cause in ["deadline", "caller_cancelled", "closed_caller", "shutdown"] {
        let fixture = Fixture::new().await;
        let limit = Arc::new(Semaphore::new(1));
        let held = limit.clone().acquire_owned().await.unwrap();
        let mut ctx = fixture.ctx();
        let (cancel, receiver) = tokio::sync::watch::channel(false);
        if cause == "caller_cancelled" {
            ctx.caller_cancel = Some(receiver);
            cancel.send(true).unwrap();
        } else if cause == "closed_caller" {
            ctx.caller_cancel = Some(receiver);
            drop(cancel);
        } else if cause == "shutdown" {
            fixture
                .console
                .state
                .shutdown
                .request("Synthetic queued shutdown");
        } else {
            ctx.request_budget =
                super::super::request_budget::RequestBudget::new(1, Duration::from_millis(20));
        }
        let error = match tokio::time::timeout(
            Duration::from_millis(300),
            Owner::begin(ctx, &fixture.console.state, limit.clone()),
        )
        .await
        .unwrap()
        {
            Ok(_) => panic!("Queued admission passed"),
            Err(error) => error,
        };
        assert_eq!(
            error.request_budget_stop_reason(),
            Some(if cause == "closed_caller" {
                "caller_cancelled"
            } else {
                cause
            })
        );
        assert!(fixture
            .local
            .list_audits(&crate::db::RequestAuditFilters::default())
            .await
            .unwrap()
            .is_empty());
        assert_eq!(fixture.balance().await, (1000, 0));
        drop(held);
        assert_eq!(limit.available_permits(), 1);
        fixture.close().await;
    }
}
