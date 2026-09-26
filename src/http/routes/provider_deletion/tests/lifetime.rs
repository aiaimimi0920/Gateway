use super::*;
use std::future::Future;

#[test]
fn runtime_teardown_releases_an_in_memory_async_wait() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let lifecycle = Arc::new(GatewayLifecycleState::default());
    let (started, ready) = oneshot::channel();
    let (destroyed, dropped) = mpsc::channel();
    struct Signal(mpsc::Sender<()>);
    impl Drop for Signal {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    let state = Arc::clone(&lifecycle);
    runtime.block_on(async {
        tokio::spawn(async move {
            run_owned::<(), _, _>(&state, || async move {
                let _resource = Signal(destroyed);
                let _ = started.send(());
                std::future::pending().await
            })
            .await
        });
        ready.await.unwrap();
    });
    runtime.shutdown_background();
    dropped.recv_timeout(Duration::from_secs(3)).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(run_owned(&lifecycle, || async { Ok(()) }))
        .unwrap();
}

#[test]
fn queued_work_survives_caller_drop_without_releasing_admission() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let lifecycle = Arc::new(GatewayLifecycleState::default());
    runtime.block_on(async {
        let (occupied, ready) = oneshot::channel();
        let (release, wait) = mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = occupied.send(());
            let _ = wait.recv_timeout(Duration::from_secs(3));
        });
        ready.await.unwrap();
        let (finished, done) = oneshot::channel();
        let mut operation = Box::pin(run_owned(&lifecycle, || async move {
            let _ = finished.send(());
            Ok(())
        }));
        let pending = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(operation.as_mut().poll(cx).is_pending())
        })
        .await;
        drop(operation);
        let rejected = run_owned(&lifecycle, || async { Ok(()) }).await;
        let _ = release.send(());
        blocker.await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), done)
            .await
            .unwrap()
            .unwrap();
        assert!(pending);
        assert_eq!(rejected.unwrap_err().http_status, Some(409));
    });
}
