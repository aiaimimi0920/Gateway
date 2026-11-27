use super::{worker::WorkerOutput, *};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{timeout, Duration, Instant};

fn success_runtime() -> Runtime {
    Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            Ok(WorkerOutput {
                usage: None,
                text: "remote fixture".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

pub(super) async fn run(
    runtime: &Runtime,
    denied: bool,
    count: Arc<AtomicUsize>,
) -> Result<Completed, GatewayError> {
    let (settled, _) = tokio::sync::watch::channel(false);
    runtime
        .execute(
            serde_json::json!({}),
            Instant::now() + Duration::from_secs(5),
            Box::new(move || {
                Box::pin(async move {
                    if denied {
                        Err(crate::protocol::cursor_remote::error(
                            "authorization_changed",
                        ))
                    } else {
                        Ok(())
                    }
                })
            }),
            Box::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            }),
            settled,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
}

#[tokio::test]
async fn cursor_remote_worker_success_denial_and_no_replay() {
    let rt = success_runtime();
    let count = Arc::new(AtomicUsize::new(0));
    let reply = run(&rt, false, count.clone()).await.unwrap();
    assert_eq!(reply.text, "remote fixture");
    assert_eq!(reply.selector, "default");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(run(&rt, true, count.clone()).await.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    rt.join().await;
    assert!(!rt.available());
}

#[tokio::test]
async fn cursor_remote_worker_drop_reaps_before_settlement_and_drain() {
    let rt = Arc::new(Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            futures::future::pending().await
        })
    }));
    let (started, observed) = oneshot::channel();
    let (settled, receiver) = tokio::sync::watch::channel(false);
    let copy = rt.clone();
    let task = tokio::spawn(async move {
        copy.execute(
            serde_json::json!({}),
            Instant::now() + Duration::from_secs(30),
            Box::new(|| Box::pin(async { Ok(()) })),
            Box::new(move || {
                let _ = started.send(());
            }),
            settled,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await
    });
    timeout(Duration::from_secs(5), observed)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    let _ = task.await;
    timeout(Duration::from_secs(5), settle(Some(&receiver)))
        .await
        .unwrap();
    timeout(Duration::from_secs(5), rt.join()).await.unwrap();
}

#[tokio::test]
async fn cursor_remote_worker_deadline_reaps_and_releases_capacity() {
    let rt = Runtime::fixture(|_input, _hooks| Box::pin(futures::future::pending()));
    let (settled, receiver) = tokio::sync::watch::channel(false);
    let result = rt
        .execute(
            serde_json::json!({}),
            Instant::now() + Duration::from_millis(150),
            Box::new(|| Box::pin(async { panic!("must not authorize") })),
            Box::new(|| panic!("must not send")),
            settled,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await;
    assert!(result.is_err());
    settle(Some(&receiver)).await;
    timeout(Duration::from_secs(2), rt.join()).await.unwrap();
}

#[tokio::test]
async fn cursor_remote_worker_missing_send_receipt_is_unknown_not_zero_execution() {
    let rt = Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            Err(crate::protocol::cursor_remote::error("worker_failed"))
        })
    });
    let (settled, receiver) = tokio::sync::watch::channel(false);
    let uncertain = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = rt
        .execute(
            serde_json::json!({}),
            Instant::now() + Duration::from_secs(5),
            Box::new(|| Box::pin(async { Ok(()) })),
            Box::new(|| panic!("no observed send")),
            settled,
            uncertain.clone(),
        )
        .await;
    assert_eq!(
        result.err().unwrap().code.as_deref(),
        Some("cursor_remote_delivery_unknown")
    );
    settle(Some(&receiver)).await;
    assert!(uncertain.load(Ordering::SeqCst));
}
