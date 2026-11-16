//! The grace retains one original future, not eligibility, a new attempt or an autonomous job.
use super::*;
#[tokio::test(start_paused = true)]
async fn inflight_check_keeps_last_admitted_slot_without_extending_deadline() {
    let budget = RequestBudget::new(1, Duration::from_secs(1));
    budget.begin_attempt().unwrap();
    assert_eq!(budget.remaining_attempts(), 0);
    assert!(budget.check_inflight().is_ok());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(
        budget
            .check_inflight()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("deadline")
    );
    let budget = RequestBudget::new(1, Duration::from_secs(10));
    budget.begin_attempt().unwrap();
    assert_eq!(
        budget
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("attempt_limit")
    );
    assert!(
        budget.check_inflight().is_err(),
        "an actual stop can never be revived"
    );
}
#[tokio::test(start_paused = true)]
async fn paid_closeout_retains_original_error_but_not_generation_eligibility() {
    let budget = RequestBudget::new(1, Duration::from_secs(1));
    let start = Instant::now();
    let result: Result<(), _> = budget
        .run(async {
            let _guard = budget.retain_deadline_closeout();
            tokio::time::sleep(Duration::from_secs(2)).await;
            assert_eq!(
                budget.check().unwrap_err().request_budget_stop_reason(),
                Some("deadline")
            );
            assert!(budget.begin_attempt().is_err());
            Err(GatewayError::server_error("original unknown accounting")
                .with_code("original_unknown"))
        })
        .await;
    let error = result.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("original_unknown"));
    assert_eq!(error.request_budget_stop_reason(), Some("deadline"));
    assert_eq!(Instant::now() - start, Duration::from_secs(2));
    assert_eq!(budget.attempts(), 0);
    assert!(!budget.0.lock().deadline_closeout);
}
#[tokio::test(start_paused = true)]
async fn paid_closeout_timeout_is_bounded_and_never_returns_late_success() {
    for delay in [2, 100] {
        let budget = RequestBudget::new(1, Duration::from_secs(1));
        let start = Instant::now();
        let result: Result<(), _> = budget
            .run(async {
                let _guard = budget.retain_deadline_closeout();
                tokio::time::sleep(Duration::from_secs(delay)).await;
                Ok(())
            })
            .await;
        assert_eq!(
            result.unwrap_err().request_budget_stop_reason(),
            Some("deadline")
        );
        assert_eq!(Instant::now() - start, Duration::from_secs(delay.min(21)));
        assert!(!budget.0.lock().deadline_closeout);
    }
}
#[tokio::test(start_paused = true)]
async fn ordinary_or_released_owner_keeps_original_deadline_drop() {
    for guarded in [false, true] {
        let budget = RequestBudget::new(1, Duration::from_secs(1));
        let start = Instant::now();
        let result: Result<(), _> = budget
            .run(async {
                if guarded {
                    drop(budget.retain_deadline_closeout());
                }
                std::future::pending().await
            })
            .await;
        assert_eq!(
            result.unwrap_err().request_budget_stop_reason(),
            Some("deadline")
        );
        assert_eq!(Instant::now() - start, Duration::from_secs(1));
    }
}
#[tokio::test]
async fn paid_closeout_task_drop_has_no_autonomous_finalizer() {
    let budget = RequestBudget::new(1, Duration::from_secs(30));
    let owned = budget.clone();
    let (entered, observed) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        owned
            .run(async {
                let _guard = owned.retain_deadline_closeout();
                entered.send(()).unwrap();
                std::future::pending::<Result<(), GatewayError>>().await
            })
            .await
    });
    observed.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(budget.stopped_reason(), Some("cancelled"));
    assert!(!budget.0.lock().deadline_closeout);
}
