//! The original paid stream can continue after handoff, but no send slot/deadline is renewed.
use super::*;
use std::time::Duration;
#[tokio::test(start_paused = true)]
async fn admitted_stream_preserves_handoff_fence_for_new_attempts_and_original_deadline() {
    let budget = RequestBudget::new(1, Duration::from_secs(1));
    assert!(budget.admitted_stream().is_err());
    let original = budget
        .run(async {
            budget.begin_attempt()?;
            budget.admitted_stream()
        })
        .await
        .unwrap();
    assert!(original.check().is_ok());
    assert!(budget.check_inflight().is_err());
    assert!(budget.begin_attempt().is_err());
    assert!(budget.admitted_stream().is_err());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(original.check().is_err());
    assert_eq!(budget.stopped_reason(), Some("deadline"));
    assert_eq!(budget.attempts(), 1);
}
#[tokio::test]
async fn admitted_stream_cannot_revive_cancellation_or_follow_a_different_attempt() {
    let budget = RequestBudget::new(2, Duration::from_secs(1));
    budget.begin_attempt().unwrap();
    let original = budget.admitted_stream().unwrap();
    budget.begin_attempt().unwrap();
    assert!(original.check().is_err());
    let current = budget.admitted_stream().unwrap();
    budget.cancel_caller();
    assert!(current.check().is_err());
    assert_eq!(budget.stopped_reason(), Some("caller_cancelled"));
}
