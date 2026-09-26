use super::*;
use futures::FutureExt;

#[tokio::test]
async fn closed_owner_rejects_new_work_but_retains_existing_reservations() {
    let owner = RunSlots::new(2);
    let permit = owner.reserve().unwrap();
    owner.close();
    assert_eq!(
        owner.reserve().unwrap_err().code.as_deref(),
        Some("freebuff_run_admission_closed")
    );
    assert_eq!(owner.slots.available_permits(), 1);
    let mut wait = Box::pin(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(wait.as_mut().now_or_never().is_none());
    drop(permit);
    wait.await.unwrap();
    assert!(owner.reserve().is_err());
}

#[tokio::test]
async fn owner_shutdown_does_not_close_an_independent_runtime() {
    let first = RunSlots::new(1);
    let second = RunSlots::new(1);
    first.wait_for_idle(Duration::from_secs(1)).await.unwrap();
    assert!(first.reserve().is_err());
    let permit = second.reserve().unwrap();
    assert_eq!(second.slots.available_permits(), 0);
    drop(permit);
    assert!(second.reserve().is_ok());
}

#[tokio::test]
async fn timed_out_wait_leaves_work_owned_and_admission_closed() {
    let owner = RunSlots::new(1);
    let permit = owner.reserve().unwrap();
    let error = owner
        .wait_for_idle(Duration::from_millis(10))
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_run_shutdown_timeout"));
    assert_eq!(owner.slots.available_permits(), 0);
    assert!(owner.reserve().is_err());
    drop(permit);
    owner.wait_for_idle(Duration::from_secs(1)).await.unwrap();
}

#[tokio::test]
async fn cancelled_shutdown_wait_does_not_reopen_admission_or_lose_permits() {
    let owner = RunSlots::new(1);
    let permit = owner.reserve().unwrap();
    let mut wait = Box::pin(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(wait.as_mut().now_or_never().is_none());
    drop(wait);
    assert!(owner.reserve().is_err());
    assert_eq!(owner.slots.available_permits(), 0);
    drop(permit);
    owner.wait_for_idle(Duration::from_secs(1)).await.unwrap();
}

#[tokio::test]
async fn concurrent_waiters_finish_only_after_the_last_reservation_returns() {
    let owner = RunSlots::new(2);
    let first = owner.reserve().unwrap();
    let last = owner.reserve().unwrap();
    let mut a = Box::pin(owner.wait_for_idle(Duration::from_secs(1)));
    let mut b = Box::pin(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(a.as_mut().now_or_never().is_none());
    assert!(b.as_mut().now_or_never().is_none());
    drop(first);
    assert!(a.as_mut().now_or_never().is_none());
    drop(last);
    let (a, b) = tokio::join!(a, b);
    a.unwrap();
    b.unwrap();
}

#[test]
fn capacity_error_and_recovery_remain_distinct_from_shutdown() {
    let owner = RunSlots::new(1);
    let permit = owner.reserve().unwrap();
    assert_eq!(
        owner.reserve().unwrap_err().code.as_deref(),
        Some("freebuff_run_capacity_exhausted")
    );
    drop(permit);
    assert!(owner.reserve().is_ok());
    owner.close();
    assert_eq!(
        owner.reserve().unwrap_err().code.as_deref(),
        Some("freebuff_run_admission_closed")
    );
}
