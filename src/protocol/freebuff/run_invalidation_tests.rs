use super::*;

#[test]
fn invalidation_rejects_new_admission_without_revoking_existing_lease() {
    let lifetime = RunLifetime::untracked();
    lifetime.acquire().unwrap();
    lifetime.invalidate();
    let error = lifetime.acquire().unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_run_invalidated"));
    assert_eq!(lifetime.inflight(), 1);
    assert_eq!(lifetime.request_count.load(Ordering::Acquire), 1);
    lifetime.release();
    assert_eq!(lifetime.inflight(), 0);
}

#[test]
fn stale_validity_observation_does_not_authorize_admission_after_invalidation() {
    let lifetime = Arc::new(RunLifetime::untracked());
    assert!(!lifetime.is_invalidated());
    let other = lifetime.clone();
    std::thread::spawn(move || other.invalidate())
        .join()
        .unwrap();
    assert!(lifetime.acquire().is_err());
    assert_eq!(lifetime.inflight(), 0);
    assert_eq!(lifetime.request_count.load(Ordering::Acquire), 0);
}

#[test]
fn racing_invalidation_balances_all_successful_admissions() {
    let lifetime = Arc::new(RunLifetime::untracked());
    let barrier = Arc::new(std::sync::Barrier::new(17));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let lifetime = lifetime.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                if lifetime.acquire().is_ok() {
                    std::thread::yield_now();
                    lifetime.release();
                    1u64
                } else {
                    0
                }
            })
        })
        .collect();
    barrier.wait();
    lifetime.invalidate();
    let accepted: u64 = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .sum();
    assert_eq!(lifetime.inflight(), 0);
    assert_eq!(lifetime.request_count.load(Ordering::Acquire), accepted);
    assert!(lifetime.acquire().is_err());
    assert_eq!(lifetime.inflight(), 0);
}

#[test]
fn valid_admission_overflow_still_fails_without_wrapping() {
    let lifetime = RunLifetime::untracked();
    lifetime.inflight.store(u64::MAX, Ordering::Release);
    let error = lifetime.acquire().unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_run_inflight_overflow")
    );
    assert_eq!(lifetime.inflight(), u64::MAX);
    assert_eq!(lifetime.request_count.load(Ordering::Acquire), 0);
}

#[test]
fn accepted_request_count_remains_saturating() {
    let lifetime = RunLifetime::untracked();
    lifetime.request_count.store(u64::MAX, Ordering::Release);
    lifetime.acquire().unwrap();
    assert_eq!(lifetime.request_count.load(Ordering::Acquire), u64::MAX);
    lifetime.release();
    assert_eq!(lifetime.inflight(), 0);
}
