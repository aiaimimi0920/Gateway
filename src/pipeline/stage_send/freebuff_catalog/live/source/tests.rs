//! Mailbox bounds, Drop and preflight delivery are proved without a detached producer.
use super::*;
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
struct Guard(Arc<AtomicBool>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
#[tokio::test]
async fn freebuff_live_source_stays_bounded_and_drops_its_original_future_without_background_work()
{
    let (sent, receiver) = tokio::sync::mpsc::channel(1);
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Guard(dropped.clone());
    let terminal = Arc::new(AtomicBool::new(false));
    let future = async move {
        let _guard = guard;
        for _ in 0..100 {
            sent.send(Delivery {
                bytes: Bytes::from_static(b"frame"),
                terminal: false,
            })
            .await
            .unwrap();
            count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    };
    let mut source = Source::new(future, receiver, terminal);
    source.prefetch().await.unwrap();
    assert_eq!(observed.load(Ordering::SeqCst), 1);
    tokio::task::yield_now().await;
    assert_eq!(observed.load(Ordering::SeqCst), 1);
    assert_eq!(
        source.next().await.unwrap().unwrap(),
        Bytes::from_static(b"frame")
    );
    assert_eq!(observed.load(Ordering::SeqCst), 1);
    assert!(source.next().await.unwrap().is_ok());
    assert_eq!(observed.load(Ordering::SeqCst), 2);
    drop(source);
    assert!(dropped.load(Ordering::Acquire));
    tokio::task::yield_now().await;
    assert_eq!(observed.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn freebuff_live_source_prefetch_does_not_confirm_terminal_consumption() {
    for consumed in [false, true] {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let terminal = Arc::new(AtomicBool::new(false));
        let future = async move {
            sender
                .send(Delivery {
                    bytes: Bytes::from_static(b"data: [DONE]\n\n"),
                    terminal: true,
                })
                .await
                .unwrap();
            Ok(())
        };
        let mut source = Source::new(future, receiver, terminal.clone());
        source.prefetch().await.unwrap();
        assert!(!terminal.load(Ordering::Acquire));
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let mut tracked = TrackedStream::new_with_error(source, move |metrics, success| {
            assert_eq!(success, consumed);
            assert_eq!(
                metrics.termination,
                Some(if consumed {
                    crate::upstream::stream::StreamTermination::Completed
                } else {
                    crate::upstream::stream::StreamTermination::Cancelled
                })
            );
            observed.fetch_add(1, Ordering::SeqCst);
        })
        .with_validated_owner_terminal(terminal);
        if consumed {
            assert!(tracked.next().await.unwrap().is_ok());
        }
        drop(tracked);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn freebuff_live_source_preserves_queued_delta_then_classified_failure_and_fuses() {
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let future = async move {
        sender
            .send(Delivery {
                bytes: Bytes::from_static(b"partial"),
                terminal: false,
            })
            .await
            .unwrap();
        Err(business(BusinessError::RunUnconfirmed))
    };
    let terminal = Arc::new(AtomicBool::new(false));
    let mut source = Source::new(future, receiver, terminal.clone());
    source.prefetch().await.unwrap();
    assert!(source.next().await.unwrap().is_ok());
    match source.next().await.unwrap().unwrap_err() {
        StreamError::Protocol(error) => assert_eq!(
            error.gateway_error().unwrap().code.as_deref(),
            Some("freebuff_business_run_unconfirmed")
        ),
        _ => panic!("protocol classification must be preserved"),
    }
    assert!(source.next().await.is_none());
    assert!(source.next().await.is_none());
    assert!(!terminal.load(Ordering::Acquire));
}
