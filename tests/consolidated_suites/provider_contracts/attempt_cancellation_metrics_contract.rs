use neuro_gateway::{
    error::GatewayError,
    retry::{execute_with_retry_after_admission_observed, RetryPolicy},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

struct TransportReleased(Arc<AtomicBool>);
impl Drop for TransportReleased {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn pending_send_is_observed_once_after_transport_release_without_a_retry() {
    let released = Arc::new(AtomicBool::new(false));
    let events = Arc::new(Mutex::new(Vec::new()));
    let out = Arc::clone(&events);
    let policy = RetryPolicy::default();
    let mut task = Box::pin(execute_with_retry_after_admission_observed(
        || {
            let lease = TransportReleased(Arc::clone(&released));
            async move {
                let _lease = lease;
                std::future::pending::<Result<(), GatewayError>>().await
            }
        },
        || async { panic!("no retry admission after cancellation") },
        |observation| {
            out.lock().unwrap().push((
                observation.succeeded,
                observation.error.and_then(|e| e.code.clone()),
                observation.is_retry,
                released.load(Ordering::SeqCst),
            ));
        },
        &policy,
    ));
    assert!(futures::poll!(task.as_mut()).is_pending());
    assert!(!released.load(Ordering::SeqCst));
    drop(task);
    assert_eq!(
        *events.lock().unwrap(),
        vec![(false, Some("attempt_cancelled".to_string()), false, true)]
    );
}

#[tokio::test(start_paused = true)]
async fn cancellation_during_readmission_or_sleep_does_not_invent_an_attempt() {
    for pending_admission in [false, true] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let out = Arc::clone(&events);
        let policy = RetryPolicy::default();
        let mut task = Box::pin(execute_with_retry_after_admission_observed(
            || async { Err::<(), _>(GatewayError::rate_limited("fixture", 5000)) },
            || async {
                if pending_admission {
                    std::future::pending::<()>().await;
                }
                Ok(())
            },
            |observation| out.lock().unwrap().push(observation.is_retry),
            &policy,
        ));
        assert!(futures::poll!(task.as_mut()).is_pending());
        if pending_admission {
            tokio::time::advance(std::time::Duration::from_secs(5)).await;
            assert!(futures::poll!(task.as_mut()).is_pending());
        }
        drop(task);
        assert_eq!(*events.lock().unwrap(), vec![false]);
    }
}
