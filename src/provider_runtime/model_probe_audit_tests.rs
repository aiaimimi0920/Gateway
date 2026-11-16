use super::{test_support::*, *};
use std::time::Duration;

#[test]
fn model_probe_audit_create_and_unread_output_drop_keep_terminal_ownership() {
    run(create_drop);
}
async fn create_drop() {
    for phase in [0, 1, 2] {
        let (fixture, local) = fixture().await;
        let pause = Arc::new(Pause::default());
        let returning = Arc::new(tokio::sync::Notify::new());
        let hooks = if phase == 0 {
            Hooks {
                before: Some(pause.clone()),
                ..Default::default()
            }
        } else {
            Hooks {
                created: Some(pause.clone()),
                returning: Some(returning.clone()),
                ..Default::default()
            }
        };
        let limit = Arc::new(Semaphore::new(1));
        let mut future: PinFuture<'_> = Box::pin(hooks.within(async {
            let _audit = ProbeAudit::begin_with_limit(&fixture.state, input(), limit.clone()).await;
            panic!("Caller unexpectedly received audit output");
        }));
        ready(&pause, &mut future).await;
        assert_eq!(limit.available_permits(), 0);
        if phase == 2 {
            // Allow task completion, but deliberately do not poll the caller's JoinHandle again.
            pause.release();
            // This executor is current-thread: no await separates the signal and task return.
            // Await only the signal, never the caller, to prove an unclaimed completed output.
            returning.notified().await;
            assert_eq!(limit.available_permits(), 0);
        }
        drop(future);
        if phase != 2 {
            pause.release();
        }
        let row = terminal(&local, "cancelled").await;
        assert_eq!(
            row.provider_account_id.as_deref(),
            Some("synthetic-provider")
        );
        assert_eq!(row.protocol_family, "bedrock_converse");
        assert_eq!(
            row.route_trace.as_ref().unwrap()["realCredentialRef"],
            "synthetic-exact"
        );
        assert!(row.total_tokens.is_none());
        close(fixture, local).await;
        assert_eq!(limit.available_permits(), 1);
    }
}
#[test]
fn model_probe_audit_recording_and_unpolled_finish_drop_are_cancellations() {
    run(record_drop);
}
async fn record_drop() {
    for unpolled_finish in [false, true] {
        let (fixture, local) = fixture().await;
        let audit = ProbeAudit::begin(&fixture.state, input()).await.unwrap();
        if unpolled_finish {
            let mut result = audit.pending.as_ref().unwrap().cancelled();
            result.status = "completed".into();
            drop(audit.finish(result));
        } else {
            drop(audit);
        }
        let row = terminal(&local, "cancelled").await;
        assert!(row.total_tokens.is_none());
        assert!(row.completed_at.is_some());
        assert!(
            row.client_disconnected_at.is_none(),
            "Unknown drop cause must not invent client disconnect"
        );
        close(fixture, local).await;
    }
}
#[test]
fn model_probe_audit_submitted_finish_survives_waiter_drop_and_drains_before_close() {
    run(finish_drop);
}
async fn finish_drop() {
    for passed in [true, false] {
        let (fixture, local) = fixture().await;
        let limit = Arc::new(Semaphore::new(1));
        let audit = ProbeAudit::begin_with_limit(&fixture.state, input(), limit.clone())
            .await
            .unwrap();
        let mut result = audit.pending.as_ref().unwrap().cancelled();
        result.status = if passed { "completed" } else { "failed" }.into();
        result.error_summary = (!passed).then(|| "Expected synthetic failure".into());
        if passed {
            result.prompt_tokens = Some(8);
            result.completion_tokens = Some(1);
            result.total_tokens = Some(9);
        }
        let pause = Arc::new(Pause::default());
        let hooks = Hooks {
            finish: Some(pause.clone()),
            ..Default::default()
        };
        let mut future: PinFuture<'_> = Box::pin(hooks.within(async {
            audit.finish(result).await.unwrap();
        }));
        ready(&pause, &mut future).await;
        drop(future);
        // Prevent the actual SQLite terminal write from completing after the task owns its result.
        let transaction = local.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        pause.release();
        tokio::task::yield_now().await;
        assert_eq!(limit.available_permits(), 0);
        let mut closing = Box::pin(local.close());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), closing.as_mut())
                .await
                .is_err()
        );
        transaction.rollback().await.unwrap();
        let row = terminal(&local, if passed { "completed" } else { "failed" }).await;
        assert_eq!(row.total_tokens, passed.then_some(9));
        assert!(row
            .error_summary
            .as_deref()
            .is_none_or(|v| !v.contains("cancelled")));
        tokio::time::timeout(Duration::from_secs(3), closing)
            .await
            .unwrap();
        assert_eq!(limit.available_permits(), 1);
        drop(fixture);
    }
}
#[test]
fn model_probe_audit_storage_failure_does_not_silently_become_cancelled() {
    run(storage_failure);
}
async fn storage_failure() {
    let (fixture, local) = fixture().await;
    let limit = Arc::new(Semaphore::new(1));
    let audit = ProbeAudit::begin_with_limit(&fixture.state, input(), limit.clone())
        .await
        .unwrap();
    let mut result = audit.pending.as_ref().unwrap().cancelled();
    result.status = "completed".into();
    // Reject only completed writes, leaving a healthy pool where an accidental cancel would succeed.
    sqlx::query("CREATE TRIGGER reject_probe_completed BEFORE UPDATE ON request_audits
        WHEN NEW.status = 'completed' BEGIN SELECT RAISE(FAIL, 'synthetic terminal rejection'); END")
        .execute(&local.pool).await.unwrap();
    assert!(audit.finish(result).await.is_err());
    let returned = tokio::time::timeout(Duration::from_secs(3), limit.acquire_owned())
        .await
        .unwrap()
        .unwrap();
    let rows = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].status, "running",
        "A failed normal write must not secretly submit cancelled"
    );
    assert!(rows[0].completed_at.is_none());
    drop(returned);
    close(fixture, local).await;
}
#[test]
fn model_probe_audit_drop_without_runtime_context_releases_ownership_without_fake_terminal() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let (fixture, local, audit, limit) = runtime.block_on(async {
                let (fixture, local) = fixture().await;
                let limit = Arc::new(Semaphore::new(1));
                let audit = ProbeAudit::begin_with_limit(&fixture.state, input(), limit.clone())
                    .await
                    .unwrap();
                (fixture, local, audit, limit)
            });
            // The runtime still exists, but this synchronous scope has no entered Handle.
            assert!(tokio::runtime::Handle::try_current().is_err());
            drop(audit);
            assert_eq!(limit.available_permits(), 1);
            runtime.block_on(async {
                let rows = local
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].status, "running");
                assert!(rows[0].completed_at.is_none());
                close(fixture, local).await;
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn model_probe_audit_admission_wait_cancellation_never_creates_a_worker_or_row() {
    run(admission);
}
async fn admission() {
    let (fixture, local) = fixture().await;
    let limit = Arc::new(Semaphore::new(1));
    let held = limit.clone().acquire_owned().await.unwrap();
    let mut pending = Box::pin(ProbeAudit::begin_with_limit(
        &fixture.state,
        input(),
        limit.clone(),
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), pending.as_mut())
            .await
            .is_err()
    );
    drop(pending);
    drop(held);
    assert_eq!(limit.available_permits(), 1);
    assert!(local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap()
        .is_empty());
    close(fixture, local).await;
}
#[test]
fn model_probe_audit_no_store_and_server_without_principal_are_not_fake_success() {
    run(unsupported_store);
}
async fn unsupported_store() {
    let (mut fixture, local) = fixture().await;
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = None;
    assert!(ProbeAudit::begin(&fixture.state, input()).await.is_none());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy(&format!(
            "postgres://synthetic:unused@{}/unused",
            listener.local_addr().unwrap()
        ))
        .unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().pg_pool = Some(pool.clone());
    assert!(ProbeAudit::begin(&fixture.state, input()).await.is_none());
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err(),
        "Missing audit identity must be rejected before network"
    );
    drop(fixture);
    pool.close().await;
    local.close().await;
}
