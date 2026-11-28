use super::*;
use crate::concurrency::aimd::{AimdConfig, AimdController};
use crate::pipeline::request_budget::RequestBudget;
use crate::upstream::stream::TrackedStream;
use bytes::Bytes;
use futures::StreamExt;

#[test]
fn optional_limit_preserves_unconfigured_accounts_and_rejects_invalid_values() {
    assert_eq!(limit_from_payload(&serde_json::json!({})).unwrap(), None);
    assert_eq!(
        limit_from_payload(&serde_json::json!({"max_concurrent_requests": null})).unwrap(),
        None
    );
    assert_eq!(
        limit_from_payload(&serde_json::json!({"max_concurrent_requests": 3})).unwrap(),
        Some(3)
    );
    for value in [
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(1001),
        serde_json::json!("3"),
    ] {
        assert!(
            limit_from_payload(&serde_json::json!({"max_concurrent_requests": value})).is_err()
        );
    }
}

#[tokio::test]
async fn three_slots_wait_then_release_and_isolate_accounts() {
    let registry = AccountConcurrency::default();
    let a = registry.gate("zzz-api", "account-a").unwrap();
    let b = registry.gate("zzz-api", "account-b").unwrap();
    let held = vec![a.acquire(3).await, a.acquire(3).await, a.acquire(3).await];
    let fourth = a.acquire(3);
    tokio::pin!(fourth);
    assert!(tokio::time::timeout(Duration::from_millis(30), &mut fourth)
        .await
        .is_err());
    let other = b.acquire(3).await;
    drop(held);
    let next = tokio::time::timeout(Duration::from_secs(1), fourth)
        .await
        .unwrap();
    drop(next);
    drop(other);
    assert_eq!(*a.active.lock(), 0);
    assert_eq!(*b.active.lock(), 0);
}

#[tokio::test]
async fn cancelled_waiter_and_retry_do_not_steal_or_leak() {
    let gate = Arc::new(LocalGate::new());
    let first = gate.acquire(1).await;
    let pending = tokio::spawn({
        let gate = Arc::clone(&gate);
        async move { gate.acquire(1).await }
    });
    tokio::task::yield_now().await;
    pending.abort();
    let _ = pending.await;
    assert_eq!(*gate.active.lock(), 1);
    drop(first);
    let retry = tokio::time::timeout(Duration::from_secs(1), gate.acquire(1))
        .await
        .unwrap();
    drop(retry);
    assert_eq!(*gate.active.lock(), 0);
}

#[tokio::test]
async fn fourth_request_does_not_reach_fake_upstream_until_a_slot_finishes() {
    let gate = Arc::new(LocalGate::new());
    let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
    let (finish, _) = tokio::sync::broadcast::channel::<usize>(4);
    let mut tasks = Vec::new();
    for id in 0..4 {
        let gate = Arc::clone(&gate);
        let sent = sent.clone();
        let mut finish = finish.subscribe();
        tasks.push(Some(tokio::spawn(async move {
            let _permit = gate.acquire(3).await;
            sent.send(id).unwrap();
            while finish.recv().await.unwrap() != id {}
        })));
    }
    let first = [
        received.recv().await.unwrap(),
        received.recv().await.unwrap(),
        received.recv().await.unwrap(),
    ];
    assert!(
        tokio::time::timeout(Duration::from_millis(30), received.recv())
            .await
            .is_err()
    );
    finish.send(first[0]).unwrap();
    tasks[first[0]].take().unwrap().await.unwrap();
    let fourth = tokio::time::timeout(Duration::from_secs(1), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fourth, 3);
    for id in first.into_iter().skip(1).chain(std::iter::once(fourth)) {
        finish.send(id).unwrap();
        tasks[id].take().unwrap().await.unwrap();
    }
    assert_eq!(*gate.active.lock(), 0);
}

#[tokio::test]
async fn wait_timeout_via_request_budget_releases_waiter_without_leaking() {
    let gate = Arc::new(LocalGate::new());
    let held = gate.acquire(1).await;
    let budget = RequestBudget::new(1, Duration::from_millis(35));
    let err = budget
        .run(async {
            let _permit = gate.acquire(1).await;
            Ok::<(), GatewayError>(())
        })
        .await
        .unwrap_err();
    assert_eq!(err.request_budget_stop_reason(), Some("deadline"));
    assert_eq!(*gate.active.lock(), 1);
    drop(held);
    let next = tokio::time::timeout(Duration::from_secs(1), gate.acquire(1))
        .await
        .unwrap();
    drop(next);
    assert_eq!(*gate.active.lock(), 0);
}

#[tokio::test]
async fn streaming_drop_and_completion_release_account_permit_exactly_once() {
    let gate = Arc::new(LocalGate::new());
    let aimd = Arc::new(AimdController::new(AimdConfig::default()));

    // Mid-stream client disconnect (Drop before EOF).
    let account_permit = AccountPermit {
        shared: None,
        local: Some(gate.acquire(1).await),
    };
    let permit = aimd.acquire().await.with_guard(account_permit);
    let tracked = TrackedStream::new_with_error(
        futures::stream::pending::<Result<Bytes, std::io::Error>>(),
        move |_metrics, _success| {
            drop(permit);
        },
    );
    assert_eq!(*gate.active.lock(), 1);
    drop(tracked);
    assert_eq!(*gate.active.lock(), 0);
    assert_eq!(aimd.snapshot().active_count, 0);

    // Normal EOF followed by Drop must not double-release.
    let account_permit = AccountPermit {
        shared: None,
        local: Some(gate.acquire(1).await),
    };
    let permit = aimd.acquire().await.with_guard(account_permit);
    let mut tracked = TrackedStream::new_with_error(
        futures::stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from_static(b"ok"))]),
        move |_metrics, _success| {
            drop(permit);
        },
    );
    assert!(tracked.next().await.unwrap().is_ok());
    assert!(tracked.next().await.is_none());
    assert_eq!(*gate.active.lock(), 0);
    drop(tracked);
    assert_eq!(*gate.active.lock(), 0);
    assert_eq!(aimd.snapshot().active_count, 0);
}

#[test]
fn pg_coordinator_helpers_skip_locally_held_keys_and_drop_safely_on_closed_channel() {
    let keys = vec![
        lock_key("zzz-api", "acc-a", 0),
        lock_key("zzz-api", "acc-a", 1),
        lock_key("zzz-api", "acc-a", 2),
    ];
    let mut held = HashSet::new();
    held.insert(keys[0]);
    assert_eq!(filter_unheld_keys(&keys, &held), vec![keys[1], keys[2]]);
    assert_eq!(
            build_try_acquire_sql(2),
            "SELECT CASE WHEN pg_try_advisory_lock($1) THEN $1::bigint WHEN pg_try_advisory_lock($2) THEN $2::bigint ELSE NULL::bigint END"
        );

    let gate = Arc::new(LocalGate::new());
    let local = {
        let mut active = gate.active.lock();
        *active += 1;
        LocalPermit(Arc::clone(&gate))
    };
    let (tx, rx) = mpsc::unbounded_channel();
    drop(rx);
    let permit = AccountPermit {
        shared: Some(SharedPermit {
            epoch: 1,
            key: keys[0],
            tx,
            armed: true,
        }),
        local: Some(local),
    };
    drop(permit);
    assert_eq!(*gate.active.lock(), 0);
}

#[test]
fn receiver_cancelled_after_shared_acquisition_releases_lock_once() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (reply_tx, reply_rx) = oneshot::channel();
    let key = lock_key("zzz-api", "account-a", 0);
    assert!(reply_tx
        .send(Ok::<_, GatewayError>(Some(SharedPermit {
            epoch: 1,
            key,
            tx,
            armed: true,
        })))
        .is_ok());
    drop(reply_rx);
    match rx.try_recv().unwrap() {
        SharedCommand::Release {
            epoch,
            key: released,
            local,
        } => {
            assert_eq!(epoch, 1);
            assert_eq!(released, key);
            assert!(local.is_none());
        }
        SharedCommand::TryAcquire { .. } => panic!("expected release"),
    }
    assert!(rx.try_recv().is_err());
}
