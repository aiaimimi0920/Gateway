//! Once claimed, a failed pre-send authorization terminates rather than replays.
use super::*;
use crate::protocol::cursor_remote::{incremental, tool_request::ToolRequest};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::{
    sync::watch,
    time::{timeout, Duration, Instant},
};

async fn phase(
    rt: &Runtime,
    request: impl Into<ToolTurn>,
    denied: bool,
    sent: Arc<AtomicUsize>,
    uncertain: Arc<AtomicBool>,
) -> Result<Completed, GatewayError> {
    let (settled, receiver) = watch::channel(false);
    let result = rt
        .execute_tools(
            "key/account/revision/ws".into(),
            request,
            json!({}),
            Instant::now() + Duration::from_secs(5),
            Box::new(move || {
                Box::pin(async move {
                    if denied {
                        Err(crate::protocol::cursor_remote::error("denied"))
                    } else {
                        Ok(())
                    }
                })
            }),
            Box::new(move || {
                sent.fetch_add(1, Ordering::SeqCst);
            }),
            settled,
            uncertain,
        )
        .await;
    timeout(Duration::from_secs(5), settle(Some(&receiver)))
        .await
        .unwrap();
    result
}

#[tokio::test]
async fn cursor_remote_incremental_authorization_failure_reaps_without_output_send_or_replay() {
    let rt = tool_tests::fixture_runtime();
    let sent = Arc::new(AtomicUsize::new(0));
    let first_request: ToolRequest = tool_tests::request(json!("hello"));
    let mut first = phase(
        &rt,
        first_request,
        false,
        sent.clone(),
        Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap();
    let guard = first.publication.take().unwrap();
    assert!(guard.publish_on_connection());
    let cap = guard.continuation("resp_first").unwrap();
    let req = crate::protocol::responses::normalize_responses(json!({
        "model":"cursor-personal","store":false,"stream":true,"previous_response_id":"resp_first",
        "input":[{"type":"function_call_output","call_id":"call_one","output":"tool output"}],
    }))
    .unwrap();
    let request = incremental::parse(&req, &cap).unwrap();
    let uncertain = Arc::new(AtomicBool::new(false));
    let error = phase(&rt, request.clone(), true, sent.clone(), uncertain.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("cursor_remote_denied"));
    assert_eq!(sent.load(Ordering::SeqCst), 1); // Only the original Run was sent.
    assert!(!uncertain.load(Ordering::SeqCst));
    timeout(Duration::from_secs(5), rt.fixture_wait_idle())
        .await
        .unwrap();
    assert!(guard.continuation("resp_first").is_none());
    // The rejected send is not parked again and cannot be retried with the old capability.
    assert!(phase(&rt, request, false, sent.clone(), uncertain)
        .await
        .is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 1);
    drop(guard);
    rt.join().await;
}
