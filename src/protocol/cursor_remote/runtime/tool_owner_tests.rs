//! Publication ownership and worker death must not strand the only remote slot.
use super::{
    tool_tests::{fixture_runtime, request, resume},
    worker::{WorkerOutput, WorkerToolCall},
    *,
};
use serde_json::json;
use std::sync::atomic::AtomicBool;
use tokio::time::{timeout, Duration, Instant};

async fn first(rt: &Runtime) -> Completed {
    execute(rt, request(json!("hello"))).await
}
async fn execute(
    rt: &Runtime,
    request: crate::protocol::cursor_remote::tool_request::ToolRequest,
) -> Completed {
    rt.execute_tools(
        "owner".into(),
        request,
        json!({}),
        Instant::now() + Duration::from_secs(5),
        Box::new(|| Box::pin(async { Ok(()) })),
        Box::new(|| {}),
        tokio::sync::watch::channel(false).0,
        Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap()
}
async fn idle(rt: &Runtime) {
    drop(
        timeout(Duration::from_secs(5), rt.slots.acquire())
            .await
            .unwrap()
            .unwrap(),
    );
}

#[tokio::test]
async fn cursor_remote_unpublished_response_cannot_resume_and_drop_releases_slot() {
    let rt = fixture_runtime();
    let unpublished = first(&rt).await;
    assert!(rt
        .execute_tools(
            "owner".into(),
            resume(),
            json!({}),
            Instant::now() + Duration::from_secs(5),
            Box::new(|| Box::pin(async { Ok(()) })),
            Box::new(|| {}),
            tokio::sync::watch::channel(false).0,
            Arc::new(AtomicBool::new(false))
        )
        .await
        .is_err());
    drop(unpublished);
    idle(&rt).await;
    drop(first(&rt).await);
    idle(&rt).await;
}

#[tokio::test]
async fn cursor_remote_paused_worker_exit_releases_slot_without_caller_result() {
    let rt = Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            let id = hooks
                .emit_tool_call(WorkerToolCall {
                    call_id: "call_one".into(),
                    name: "echo".into(),
                    arguments: "{}".into(),
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(80)) => {
                    Err(crate::protocol::cursor_remote::error("remote_end_during_tool_wait"))
                }
                _ = hooks.recv_tool_result(&id) => unreachable!("no caller result sent"),
            }
        })
    });
    let mut result = first(&rt).await;
    result.publication.as_mut().unwrap().commit();
    idle(&rt).await;
    assert!(rt.available());
    drop(result);
    rt.join().await;
}

#[tokio::test]
async fn cursor_remote_connection_drop_reaps_a_published_pause() {
    let rt = fixture_runtime();
    let mut result = first(&rt).await;
    let guard = result.publication.take().unwrap();
    assert!(guard.can_publish());
    assert!(guard.publish_on_connection());
    drop(guard);
    idle(&rt).await;
    assert!(rt.available());
    drop(first(&rt).await);
    idle(&rt).await;
    rt.join().await;
}

#[tokio::test]
async fn cursor_remote_old_connection_epoch_cannot_cancel_later_pause() {
    let rt = Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            let _ = hooks
                .tool(WorkerToolCall {
                    call_id: "call_one".into(),
                    name: "echo".into(),
                    arguments: r#"{"value":"hello"}"#.into(),
                    text: "prelude".into(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            hooks.continued()?;
            let _ = hooks
                .tool(WorkerToolCall {
                    call_id: "call_two".into(),
                    name: "echo".into(),
                    arguments: "{}".into(),
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            hooks.continued()?;
            Ok(WorkerOutput {
                usage: None,
                text: "done".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    });
    let mut first = first(&rt).await;
    let old = first.publication.take().unwrap();
    assert!(old.publish_on_connection());
    let mut second = execute(&rt, resume()).await;
    let current = second.publication.take().unwrap();
    assert!(!old.can_publish());
    assert!(!old.publish_on_connection());
    assert!(current.publish_on_connection());
    drop(old);
    let mut history = resume().history;
    history.push(second.call.unwrap().history_item());
    history.push(json!({"type":"function_call_output","call_id":"call_two","output":"ok"}));
    assert_eq!(execute(&rt, request(json!(history))).await.text, "done");
    drop(current);
    idle(&rt).await;
    rt.join().await;
}

#[tokio::test]
async fn cursor_remote_closed_pause_cannot_be_published() {
    let rt = fixture_runtime();
    let mut result = first(&rt).await;
    let guard = result.publication.take().unwrap();
    assert!(guard.can_publish());
    rt.close();
    assert!(!guard.can_publish());
    assert!(!guard.publish_on_connection());
    drop(guard);
    timeout(Duration::from_secs(5), rt.join()).await.unwrap();
}

#[tokio::test]
async fn cursor_remote_expired_tool_resume_preserves_published_pause() {
    let rt = fixture_runtime();
    let mut paused = first(&rt).await;
    assert!(paused.publication.as_mut().unwrap().commit());
    let result = rt
        .execute_tools(
            "owner".into(),
            resume(),
            json!({}),
            Instant::now() - Duration::from_secs(1),
            Box::new(|| panic!("expired request must not authorize")),
            Box::new(|| panic!("expired request must not send")),
            tokio::sync::watch::channel(false).0,
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("cursor_remote_deadline")
    );
    assert_eq!(execute(&rt, resume()).await.text, "final answer");
    idle(&rt).await;
    rt.join().await;
}
