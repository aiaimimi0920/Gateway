use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
use tokio::sync::oneshot;

struct ReleaseOnDrop(Option<mpsc::Sender<()>>);
impl ReleaseOnDrop {
    fn release(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.release();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_running_solver_retains_input_and_capacity() {
    let slots = Arc::new(Semaphore::new(1));
    let worker_slots = slots.clone();
    let input = Arc::new([42; 64]);
    let weak = Arc::downgrade(&input);
    let (started, ready) = oneshot::channel();
    let (release, released) = mpsc::channel();
    let mut release = ReleaseOnDrop(Some(release));
    let (finished, completion) = oneshot::channel();
    let caller = tokio::spawn(async move {
        run_with(worker_slots, || {
            move || {
                started.send(()).unwrap();
                let released = released.recv_timeout(Duration::from_secs(5)).is_ok();
                assert_eq!(input[0], 42);
                let _ = finished.send(());
                if released {
                    Ok(())
                } else {
                    Err(GatewayError::server_error("fixture release timed out"))
                }
            }
        })
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    // Reaching this point while computation is blocked proves core-worker progress.
    assert!(
        !caller.is_finished(),
        "solver must remain blocked until test releases it"
    );
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert_eq!(slots.available_permits(), 0);
    assert!(weak.upgrade().is_some());
    release.release();
    tokio::time::timeout(Duration::from_secs(5), completion)
        .await
        .unwrap()
        .unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(5), slots.clone().acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert!(weak.upgrade().is_none());
    assert_eq!(slots.available_permits(), 1);
}

#[test]
fn cancelled_queued_solver_does_not_execute() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let (release, released) = mpsc::channel();
    let mut release = ReleaseOnDrop(Some(release));
    runtime.block_on(async {
        let (started, ready) = oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            released
                .recv_timeout(Duration::from_secs(5))
                .expect("release blocking pool");
        });
        tokio::time::timeout(Duration::from_secs(5), ready)
            .await
            .unwrap()
            .unwrap();
        let slots = Arc::new(Semaphore::new(1));
        let worker_slots = slots.clone();
        let ran = Arc::new(AtomicBool::new(false));
        let worker_ran = ran.clone();
        let (prepared, ready) = oneshot::channel();
        let caller = tokio::spawn(async move {
            run_with(worker_slots, || {
                prepared.send(()).unwrap();
                move || {
                    worker_ran.store(true, Ordering::SeqCst);
                    Ok(())
                }
            })
            .await
        });
        tokio::time::timeout(Duration::from_secs(5), ready)
            .await
            .unwrap()
            .unwrap();
        caller.abort();
        assert!(
            caller.await.unwrap_err().is_cancelled(),
            "caller must be waiting for the queued solver"
        );
        release.release();
        tokio::time::timeout(Duration::from_secs(5), blocker)
            .await
            .unwrap()
            .unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(5), slots.clone().acquire_owned())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert!(!ran.load(Ordering::SeqCst));
        assert_eq!(slots.available_permits(), 1);
    });
}
