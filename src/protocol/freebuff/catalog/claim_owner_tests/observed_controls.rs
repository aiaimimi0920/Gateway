//! A stalled observer belongs to the original future, not a spawned producer or a detached FINISH.
use super::observed_stream::{facts, stream};
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
fn partial() -> fixture::Spec {
    stream(
        vec![
            json!({"choices":[{"delta":{"content":"partial"}}],"usage":{"cost":0.125}}),
            json!({"choices":[{"finish_reason":"stop"}],"usage":{"cost":0.25}}),
        ],
        true,
    )
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_cancel_and_deadline_interrupt_backpressure_before_next_frame(
) {
    for deadline in [false, true] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = completion::activate(&db, &requests, &peer).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        peer.push(partial());
        peer.push(fixture::Spec::json(204, Value::Null));
        with_test_origin(peer.origin.clone(), async {
            let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
            let seen = Arc::new(tokio::sync::Notify::new()); let notify = seen.clone();
            let dropped = Arc::new(AtomicBool::new(false)); let guard = dropped.clone();
            let (cancel, cancelled) = tokio::sync::oneshot::channel();
            let mut cancel = Some(cancel);
            let step = run.execute_step_observed(multistep::intent(), 4096,
                if deadline { Duration::from_millis(500) } else { BUDGET }, async { let _ = cancelled.await; }, move |_| {
                let notify = notify.clone(); let guard = Dropped(guard.clone());
                async move { let _guard = guard; notify.notify_one(); std::future::pending::<()>().await; Ok(()) }
            });
            tokio::pin!(step);
            tokio::select! { result = &mut step => panic!("observer never received partial output: {:?}", result.completion()), _ = seen.notified() => {} }
            assert_eq!(facts(&db, &session).await["lastStep"]["cost"], 0.125);
            if !deadline { cancel.take().unwrap().send(()).unwrap(); }
            let decoded = step.await;
            assert_eq!(decoded.completion().err(), Some(if deadline {BusinessError::Timeout} else {BusinessError::Cancelled}));
            assert!(dropped.load(Ordering::SeqCst));
            assert_eq!(facts(&db, &session).await["lastStep"]["cost"], 0.125);
            assert_eq!(peer.count(), 3);
            assert_eq!(decoded.finish_run(BUDGET).await.unwrap().totals.total_credits, 13);
            drop(cancel);
        }).await;
        let finish: Value = serde_json::from_slice(&peer.hits.lock().unwrap()[3].body).unwrap();
        assert_eq!(
            finish["status"],
            if deadline { "failed" } else { "cancelled" }
        );
        drop(owner);
        peer.close().await;
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_cancel_during_cost_write_blocks_callback_and_finish() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(partial());
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let resume = Arc::new(tokio::sync::Notify::new()); let pause = resume.clone();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let step = run.execute_step_observed(multistep::intent(), 4096, BUDGET, async { let _ = cancelled.await; }, move |frame| {
            let tx = tx.clone(); let pause = pause.clone();
            async move {
                tx.send(frame).await.unwrap();
                pause.notified().await;
                Ok(())
            }
        });
        tokio::pin!(step);
        tokio::select! { _ = &mut step => panic!("first observer must be pending"), event = rx.recv() => assert!(event.is_some()) }
        let writer = db.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        resume.notify_one();
        tokio::select! {
            _ = &mut step => panic!("new observation must wait for SQLite"),
            _ = rx.recv() => panic!("callback crossed an uncommitted cost write"),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {},
        }
        cancel.send(()).unwrap();
        let decoded = step.await;
        assert_eq!(decoded.completion().err(), Some(BusinessError::RunJournalUnavailable));
        assert_eq!(decoded.finish_run(BUDGET).await.err(), Some(BusinessError::RunJournalUnavailable));
        writer.commit().await.unwrap();
        assert_eq!(peer.count(), 3, "uncertain cost commit cannot send FINISH");
    }).await;
    drop(owner);
    peer.close().await;
    // This deliberately cancels a queued SQLite command, whose native worker can outlive pool
    // shutdown. Do not change production busy_timeout or hide a leak with a fixed sleep: await
    // actual exclusive file access, bounded by the runtime's ten-second busy wait plus cleanup.
    db.close().await;
    drop(db);
    #[cfg(windows)]
    tokio::time::timeout(Duration::from_secs(12), async {
        use std::os::windows::fs::OpenOptionsExt;
        loop {
            let released = [
                "runtime.sqlite3",
                "runtime.sqlite3-wal",
                "runtime.sqlite3-shm",
            ]
            .iter()
            .all(|name| {
                match std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(root.join(name))
                {
                    Ok(_) => true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
                    Err(error) if error.raw_os_error() == Some(32) => false,
                    Err(error) => panic!("Cannot inspect isolated SQLite worker release: {error}"),
                }
            });
            if released {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("cancelled SQLite worker must release its native file handles");
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_ready_observer_cancellation_stops_same_chunk() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(partial());
    peer.push(fixture::Spec::json(204, Value::Null));
    with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let signal = Arc::new(tokio::sync::Notify::new());
        let stop = signal.clone();
        let mut count = 0;
        let decoded = run
            .execute_step_observed(multistep::intent(), 4096, BUDGET, signal.notified(), |_| {
                count += 1;
                stop.notify_one();
                std::future::ready(Ok(()))
            })
            .await;
        assert_eq!(count, 1);
        assert_eq!(decoded.completion().err(), Some(BusinessError::Cancelled));
        assert_eq!(
            decoded
                .finish_run(BUDGET)
                .await
                .unwrap()
                .totals
                .total_credits,
            13
        );
    })
    .await;
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_observed_stream_drop_retains_durable_cost_with_no_background_finish() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = completion::activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let mut spec = partial();
    let _eof = spec.hold_eof();
    peer.push(spec);
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = dropped.clone();
    let before = with_test_origin(peer.origin.clone(), async {
        let run = owner.start_catalog_run(&requests, BUDGET).await.unwrap();
        let seen = Arc::new(tokio::sync::Notify::new()); let notify = seen.clone();
        let step = run.execute_step_observed(multistep::intent(), 4096, BUDGET, std::future::pending(), move |_| {
            let notify = notify.clone(); let guard = Dropped(guard.clone());
            async move { let _guard = guard; notify.notify_one(); std::future::pending::<()>().await; Ok(()) }
        });
        let mut step = Box::pin(step);
        tokio::select! { _ = &mut step => panic!("observer must be pending"), _ = seen.notified() => {} }
        let before = facts(&db, &session).await;
        assert_eq!(before["phase"], "model_observed");
        drop(step);
        assert!(dropped.load(Ordering::SeqCst));
        peer.wait_drops(3).await;
        before
    }).await;
    assert_eq!(row(&db).await, original);
    drop(owner);
    db.close().await;
    let db = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(facts(&db, &session).await, before);
    assert_eq!(peer.count(), 3, "Drop/reopen never FINISH/end/replay");
    peer.close().await;
    cleanup(&root, db).await;
}
