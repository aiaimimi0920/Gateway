use super::*;
use crate::provider_identity::ProviderAliases;

fn aliases(pairs: &[(&str, &str)]) -> ProviderAliases {
    ProviderAliases::new(
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
    )
    .unwrap()
}

#[test]
fn publication_reuses_existing_owner_and_provider_limit_in_both_directions() {
    for initial in ["old", "new"] {
        let queue = Arc::new(WaitRegistry::default());
        let first = queue.enter("first".into(), initial.into(), 10).unwrap();
        let gate = first.inner.gate.clone();
        queue
            .reconcile_identities(&aliases(&[("old", "new")]))
            .unwrap();
        let mut leases = vec![first];
        for i in 1..MAX_PER_PROVIDER {
            let id = if i % 2 == 0 { "old" } else { "new" };
            let lease = queue.enter(format!("key-{i}"), id.into(), 10).unwrap();
            assert!(Arc::ptr_eq(&gate, &lease.inner.gate));
            leases.push(lease);
        }
        for id in ["old", "new"] {
            assert!(queue.enter("overflow".into(), id.into(), 1).is_err());
        }
        assert_eq!(queue.0.lock().providers.len(), 1);
        drop(leases);
        let e = queue.0.lock();
        assert_eq!(
            (e.count, e.bytes, e.keys.len(), e.providers.len()),
            (0, 0, 0, 0)
        );
    }
}

#[test]
fn conflicting_live_queues_fail_atomically_without_merging_or_losing_counts() {
    let queue = Arc::new(WaitRegistry::default());
    let a = queue.enter("a".into(), "old".into(), 11).unwrap();
    let b = queue.enter("b".into(), "new".into(), 17).unwrap();
    let c = queue.enter("c".into(), "z-new".into(), 23).unwrap();
    let error = queue
        .reconcile_identities(&aliases(&[("old", "new"), ("z-old", "z-new")]))
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("recovery_wait_identity_conflict")
    );
    {
        let e = queue.0.lock();
        assert_eq!((e.count, e.bytes, e.providers.len()), (3, 51, 3));
        assert_eq!(e.identities.pairs().count(), 0);
        assert!(Arc::ptr_eq(&a.inner.gate, &e.providers["old"].1));
        assert!(Arc::ptr_eq(&b.inner.gate, &e.providers["new"].1));
        assert!(e.providers.contains_key("z-new"));
    }
    drop((a, b, c));
    assert_eq!(queue.0.lock().count, 0);
}

#[test]
fn stale_snapshots_do_not_erase_names_and_retargeting_is_rejected() {
    let queue = Arc::new(WaitRegistry::default());
    queue
        .reconcile_identities(&aliases(&[("old", "new")]))
        .unwrap();
    queue
        .reconcile_identities(&ProviderAliases::default())
        .unwrap();
    queue
        .reconcile_identities(&aliases(&[("old", "new")]))
        .unwrap();
    for pairs in [[("old", "other")], [("new", "other")], [("other", "new")]] {
        assert!(queue.reconcile_identities(&aliases(&pairs)).is_err());
    }
    let a = queue.enter("a".into(), "old".into(), 1).unwrap();
    let b = queue.enter("b".into(), "new".into(), 1).unwrap();
    assert!(Arc::ptr_eq(&a.inner.gate, &b.inner.gate));
}

#[tokio::test]
async fn fifo_and_cancelled_waiter_keep_the_original_lock_across_publication() {
    let queue = Arc::new(WaitRegistry::default());
    let first = queue.enter("first".into(), "old".into(), 1).unwrap();
    let turn = first.turn().await;
    let second = queue.enter("second".into(), "old".into(), 1).unwrap();
    let mut second_turn = Box::pin(second.turn());
    tokio::select! { biased; _ = &mut second_turn => panic!("lock was held"), _ = async {} => {} }
    queue
        .reconcile_identities(&aliases(&[("old", "new")]))
        .unwrap();
    let third = queue.enter("third".into(), "new".into(), 1).unwrap();
    let mut third_turn = Box::pin(third.turn());
    tokio::select! { biased; _ = &mut third_turn => panic!("new name bypassed lock"), _ = async {} => {} }
    drop(turn);
    let turn = second_turn.await;
    tokio::select! { biased; _ = &mut third_turn => panic!("FIFO was bypassed"), _ = async {} => {} }
    drop(third_turn);
    drop(third);
    drop(turn);
    let last = queue.enter("last".into(), "new".into(), 1).unwrap();
    let last_turn = tokio::time::timeout(std::time::Duration::from_secs(1), last.turn())
        .await
        .unwrap();
    drop((last_turn, last, first, second));
    assert_eq!(queue.0.lock().count, 0);
}

#[tokio::test]
async fn turn_outliving_lease_retains_owner_until_unlock() {
    let queue = Arc::new(WaitRegistry::default());
    let first = queue.enter("key".into(), "new".into(), 19).unwrap();
    let turn = first.turn().await;
    drop(first);
    queue
        .reconcile_identities(&aliases(&[("old", "new")]))
        .unwrap();
    assert_eq!(queue.0.lock().bytes, 19);
    let second = queue.enter("other".into(), "old".into(), 5).unwrap();
    let mut next = Box::pin(second.turn());
    tokio::select! { biased; _ = &mut next => panic!("owner was released early"), _ = async {} => {} }
    drop(turn);
    let next = next.await;
    drop((next, second));
    let e = queue.0.lock();
    assert_eq!((e.count, e.bytes, e.providers.len()), (0, 0, 0));
}

#[test]
fn registered_names_remain_bounded_across_incremental_publications() {
    let queue = WaitRegistry::default();
    let full = ProviderAliases::new(
        (0..256)
            .map(|i| (format!("old-{i}"), format!("new-{i}")))
            .collect(),
    )
    .unwrap();
    queue.reconcile_identities(&full).unwrap();
    assert!(queue
        .reconcile_identities(&aliases(&[("extra", "extra-api")]))
        .is_err());
    assert_eq!(queue.0.lock().identities.pairs().count(), 256);
}
