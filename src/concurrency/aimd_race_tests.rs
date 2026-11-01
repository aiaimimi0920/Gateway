//! Concurrent feedback must never withhold the same capacity twice.
use super::*;
use std::sync::Barrier;

fn controller() -> Arc<AimdController> {
    Arc::new(AimdController::new(AimdConfig {
        initial_limit: 16,
        max_limit: 32,
        increase_threshold: 1,
        ..Default::default()
    }))
}

fn assert_capacity(controller: &Arc<AimdController>) {
    let snapshot = controller.snapshot();
    let permits: Vec<_> = std::iter::from_fn(|| controller.try_acquire()).collect();
    assert_eq!(snapshot.active_count, 0);
    assert_eq!(permits.len(), snapshot.current_limit);
    assert_eq!(snapshot.available, permits.len());
    assert!(
        !permits.is_empty(),
        "controller cannot recover from zero capacity"
    );
    drop(permits);
}

#[test]
fn simultaneous_failures_and_mixed_feedback_preserve_actual_capacity() {
    for mixed in [false, true] {
        let barrier = Arc::new(Barrier::new(9));
        let controller = Arc::new(Mutex::new(controller()));
        let mut mismatches = 0;
        std::thread::scope(|scope| {
            for worker in 0..8 {
                let barrier = barrier.clone();
                let controller = controller.clone();
                scope.spawn(move || {
                    for _ in 0..1000 {
                        barrier.wait();
                        let controller = controller.lock().clone();
                        if mixed && worker % 2 == 0 {
                            controller.on_success();
                        } else {
                            controller.on_failure(FailureKind::General);
                        }
                        barrier.wait();
                    }
                });
            }
            for _ in 0..1000 {
                barrier.wait();
                barrier.wait();
                // Do not panic while worker threads still need their next barrier.
                let ctrl = controller.lock().clone();
                let snapshot = ctrl.snapshot();
                let held: Vec<_> = std::iter::from_fn(|| ctrl.try_acquire()).collect();
                if held.len() != snapshot.current_limit
                    || held.len() != snapshot.available
                    || held.is_empty()
                {
                    mismatches += 1;
                }
                drop(held);
                *controller.lock() = self::controller();
            }
        });
        assert_eq!(
            mismatches, 0,
            "feedback drifted from actual semaphore capacity"
        );
    }
}

#[tokio::test]
async fn pending_withholding_and_cancelled_waiters_release_capacity() {
    let controller = controller();
    let held: Vec<_> = (0..16).map(|_| controller.try_acquire().unwrap()).collect();
    controller.on_failure(FailureKind::RateLimited);
    assert_eq!(controller.snapshot().available, 0);
    let mut waiter = Box::pin(controller.acquire());
    assert!(futures::poll!(&mut waiter).is_pending());
    drop(waiter);
    controller.on_success();
    drop(held);
    assert_capacity(&controller);
}
