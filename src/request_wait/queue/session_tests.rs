//! Session fairness never discounts real retained requests, bytes or FIFO ownership.
use super::*;

fn enter(
    queue: &Arc<WaitRegistry>,
    key: &str,
    session: u8,
    provider: &str,
    bytes: usize,
) -> WaitLease {
    queue
        .enter_session(key.into(), Some([session; 32]), provider.into(), bytes)
        .unwrap()
}

#[test]
fn exactly_sixteen_sessions_per_key_and_duplicates_share_only_one_slot() {
    let queue = Arc::new(WaitRegistry::default());
    let mut sessions: Vec<_> = (0..16)
        .map(|id| enter(&queue, "key", id, "p", 10))
        .collect();
    assert_eq!(queue.0.lock().keys["key"].slots, 16);
    assert_eq!(
        queue.0.lock().full_reason("key", Some(&[16; 32]), "p", 10),
        Some("key_slots")
    );
    assert_eq!(
        queue
            .enter_session("key".into(), Some([16; 32]), "p".into(), 10)
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("recovery_wait_queue_full")
    );
    let duplicate = enter(&queue, "key", 0, "p", 11);
    assert_eq!(queue.0.lock().keys["key"].slots, 16);
    {
        let entries = queue.0.lock();
        assert_eq!((entries.count, entries.bytes), (17, 171));
    }
    drop(sessions.remove(0));
    assert_eq!(
        queue.0.lock().keys["key"].slots,
        16,
        "one remaining request still owns the session"
    );
    drop(duplicate);
    let replacement = enter(&queue, "key", 16, "p", 10);
    drop((sessions, replacement));
    let entries = queue.0.lock();
    assert_eq!(
        (
            entries.count,
            entries.bytes,
            entries.keys.len(),
            entries.providers.len()
        ),
        (0, 0, 0, 0)
    );
}

#[test]
fn anonymous_requests_still_need_individual_slots_and_finished_requests_do_not_accumulate() {
    let queue = Arc::new(WaitRegistry::default());
    for _ in 0..100 {
        let request = enter(&queue, "key", 1, "p", 7);
        assert_eq!(queue.0.lock().keys["key"].slots, 1);
        drop(request);
        assert_eq!(queue.0.lock().count, 0);
    }
    let leases: Vec<_> = (0..16)
        .map(|_| queue.enter("key".into(), "p".into(), 1).unwrap())
        .collect();
    assert!(queue.enter("key".into(), "p".into(), 1).is_err());
    drop(leases);
    assert!(queue.0.lock().keys.is_empty());
}

#[test]
fn same_label_is_scoped_to_the_key_without_merging_provider_fifo_or_bytes() {
    let queue = Arc::new(WaitRegistry::default());
    let first = enter(&queue, "a", 1, "p", 13);
    let other_key = enter(&queue, "b", 1, "p", 17);
    let other_provider = enter(&queue, "a", 1, "q", 19);
    let entries = queue.0.lock();
    assert_eq!((entries.keys["a"].slots, entries.keys["b"].slots), (1, 1));
    assert_eq!(
        (entries.count, entries.bytes, entries.providers.len()),
        (3, 49, 2)
    );
    assert!(!Arc::ptr_eq(&first.inner.gate, &other_provider.inner.gate));
    drop(entries);
    drop((first, other_key, other_provider));
    assert!(queue.0.lock().keys.is_empty());
}

#[test]
fn same_session_cannot_bypass_provider_global_or_memory_limits() {
    let queue = Arc::new(WaitRegistry::default());
    let mut leases = Vec::new();
    for provider in ["p", "q"] {
        for _ in 0..32 {
            leases.push(enter(&queue, "key", 1, provider, 1));
        }
        if provider == "p" {
            assert_eq!(
                queue.0.lock().full_reason("key", Some(&[1; 32]), "p", 1),
                Some("provider_requests")
            );
            assert!(queue
                .enter_session("key".into(), Some([1; 32]), "p".into(), 1)
                .is_err());
        }
    }
    assert_eq!(queue.0.lock().keys["key"].slots, 1);
    assert_eq!(
        queue.0.lock().full_reason("key", Some(&[1; 32]), "r", 1),
        Some("global_requests")
    );
    assert!(queue
        .enter_session("key".into(), Some([1; 32]), "r".into(), 1)
        .is_err());
    drop(leases);
    let all_memory = enter(&queue, "key", 1, "p", MAX_BYTES);
    assert_eq!(
        queue.0.lock().full_reason("key", Some(&[1; 32]), "p", 1),
        Some("retained_bytes")
    );
    assert!(queue
        .enter_session("key".into(), Some([1; 32]), "p".into(), 1)
        .is_err());
    drop(all_memory);
    assert_eq!(queue.0.lock().bytes, 0);
}

#[tokio::test]
async fn turn_retains_session_slot_until_unlock_and_cancelled_waiter_releases_its_reference() {
    let queue = Arc::new(WaitRegistry::default());
    let first = enter(&queue, "key", 1, "p", 1);
    let turn = first.turn().await;
    let duplicate = enter(&queue, "key", 1, "p", 2);
    let waiter = tokio::spawn(async move { duplicate.turn().await });
    tokio::task::yield_now().await;
    assert!(!waiter.is_finished());
    waiter.abort();
    assert!(matches!(waiter.await, Err(error) if error.is_cancelled()));
    drop(first);
    assert_eq!(queue.0.lock().keys["key"].slots, 1);
    assert_eq!(queue.0.lock().count, 1);
    drop(turn);
    assert!(queue.0.lock().keys.is_empty());
}
