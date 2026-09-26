use std::future::{poll_fn, Future};
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use super::DebounceTimer;

async fn poll_once<F: Future>(future: F) -> Poll<F::Output> {
    tokio::pin!(future);
    poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await
}

async fn expired_timer() -> DebounceTimer {
    let mut timer = DebounceTimer::default();
    timer.schedule(Duration::from_millis(10));
    let sleep = timer.sleep.as_mut().unwrap();
    let deadline = sleep.deadline();
    // Register the actual Sleep, but leave its readiness unconsumed by the owner.
    let _ = poll_once(sleep.as_mut()).await;
    tokio::time::sleep_until(deadline).await;
    assert!(timer.sleep.as_ref().unwrap().is_elapsed());
    timer
}

#[tokio::test]
async fn folder_sync_expired_timer_cannot_cross_disable() {
    let mut timer = expired_timer().await;
    timer.cancel();
    assert!(poll_once(timer.ready()).await.is_pending());
}

#[tokio::test]
async fn folder_sync_expired_timer_cannot_consume_new_window() {
    let mut timer = expired_timer().await;
    timer.cancel();
    timer.schedule(Duration::from_secs(3600));
    let deadline = timer.sleep.as_ref().unwrap().deadline();
    assert!(poll_once(timer.ready()).await.is_pending());
    assert_eq!(timer.sleep.as_ref().unwrap().deadline(), deadline);
}

#[tokio::test]
async fn folder_sync_later_events_keep_first_deadline() {
    let mut timer = DebounceTimer::default();
    timer.schedule(Duration::from_millis(10));
    let first_deadline = timer.sleep.as_ref().unwrap().deadline();
    timer.schedule(Duration::from_secs(3600));
    assert_eq!(timer.sleep.as_ref().unwrap().deadline(), first_deadline);
    tokio::time::timeout(Duration::from_secs(1), timer.ready())
        .await
        .expect("later events postponed the first window");
}

#[tokio::test]
async fn folder_sync_cancelled_select_wait_retains_deadline() {
    let mut timer = DebounceTimer::default();
    timer.schedule(Duration::from_secs(3600));
    let deadline = timer.sleep.as_ref().unwrap().deadline();
    assert!(poll_once(timer.ready()).await.is_pending());
    // Dropping the ready() future models another select branch winning.
    assert_eq!(timer.sleep.as_ref().unwrap().deadline(), deadline);
    assert!(poll_once(timer.ready()).await.is_pending());
}

#[tokio::test]
async fn folder_sync_timer_completes_only_once() {
    let mut timer = DebounceTimer::default();
    timer.schedule(Duration::ZERO);
    tokio::time::timeout(Duration::from_secs(1), timer.ready())
        .await
        .unwrap();
    assert!(poll_once(timer.ready()).await.is_pending());
}

struct WakeProbe;

impl Wake for WakeProbe {
    fn wake(self: Arc<Self>) {}
}

fn register_waker(timer: &mut DebounceTimer) -> Weak<WakeProbe> {
    let probe = Arc::new(WakeProbe);
    let weak = Arc::downgrade(&probe);
    let waker = Waker::from(probe);
    let mut context = Context::from_waker(&waker);
    let ready = timer.ready();
    tokio::pin!(ready);
    assert!(ready.as_mut().poll(&mut context).is_pending());
    weak
}

#[tokio::test]
async fn folder_sync_disable_releases_registered_timer_waker() {
    let mut timer = DebounceTimer::default();
    timer.schedule(Duration::from_secs(3600));
    let weak = register_waker(&mut timer);
    assert!(weak.upgrade().is_some(), "sleep did not register the probe");
    timer.cancel();
    assert!(
        weak.upgrade().is_none(),
        "disabled timer retains task waker"
    );
}

#[tokio::test]
async fn folder_sync_owner_drop_releases_registered_timer_waker() {
    let mut timer = DebounceTimer::default();
    timer.schedule(Duration::from_secs(3600));
    let weak = register_waker(&mut timer);
    assert!(weak.upgrade().is_some(), "sleep did not register the probe");
    drop(timer);
    assert!(weak.upgrade().is_none(), "orphan timer retains task waker");
}
