//! In-process pure-Rust worker boundary checks; zero external service calls.
use super::{
    tests::run,
    worker::{build_completed, WorkerOutput, WorkerToolCall},
    *,
};
use crate::protocol::cursor_remote::thinking::{MAX_PHASE, MAX_RUN};
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn cursor_thinking_partial_worker_success_never_bypasses_eof_or_body_requirement() {
    for oversize in [false, true] {
        let rt = Runtime::fixture(move |_input, hooks| {
            Box::pin(async move {
                hooks.authorize().await?;
                hooks.sent()?;
                if oversize {
                    Ok(WorkerOutput {
                        usage: None,
                        text: "answer".into(),
                        thinking: Some("x".repeat(MAX_PHASE + 1)),
                        requested_model: "default".into(),
                    })
                } else {
                    Ok(WorkerOutput {
                        usage: None,
                        text: String::new(),
                        thinking: Some("only public thinking".into()),
                        requested_model: "default".into(),
                    })
                }
            })
        });
        let count = Arc::new(AtomicUsize::new(0));
        assert!(run(&rt, false, count.clone()).await.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        tokio::time::timeout(std::time::Duration::from_secs(5), rt.fixture_wait_idle())
            .await
            .unwrap();
        rt.join().await;
    }
}

#[test]
fn cursor_worker_thinking_is_optional_bounded_and_cannot_replace_answer() {
    let mut total = 0;
    assert!(
        build_completed("answer".into(), None, "default".into(), None, &mut total)
            .unwrap()
            .thinking
            .is_none()
    );
    let value = build_completed(
        "answer".into(),
        Some("公开".into()),
        "default".into(),
        None,
        &mut total,
    )
    .unwrap();
    assert_eq!(value.thinking.unwrap().text(), "公开");
    assert!(build_completed(
        String::new(),
        Some("公开".into()),
        "default".into(),
        None,
        &mut total
    )
    .is_err());
    assert!(build_completed(
        "answer".into(),
        Some("x".repeat(MAX_PHASE + 1)),
        "default".into(),
        None,
        &mut total
    )
    .is_err());

    let mut run_total = 0;
    for _ in 0..MAX_RUN / MAX_PHASE {
        let result = build_completed(
            String::new(),
            Some("x".repeat(MAX_PHASE)),
            "default".into(),
            Some(("c".into(), "echo".into(), "{}".into())),
            &mut run_total,
        )
        .unwrap();
        assert!(result.call.is_some());
        assert!(result.text.is_empty());
    }
    assert!(build_completed(
        String::new(),
        Some("x".repeat(MAX_PHASE)),
        "default".into(),
        Some(("c".into(), "echo".into(), "{}".into())),
        &mut run_total,
    )
    .is_err());
}

#[tokio::test]
async fn cursor_thinking_valid_tool_handoff_then_unsolicited_grant_reaps_worker() {
    use serde_json::json;
    use tokio::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    let handler: TestWorker = Arc::new(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            let _ = hooks
                .emit_tool_call(WorkerToolCall {
                    call_id: "c".into(),
                    name: "echo".into(),
                    arguments: "{}".into(),
                    text: String::new(),
                    thinking: Some("public".into()),
                    requested_model: "default".into(),
                })
                .await?;
            // Unsolicited continued() without receiving a caller grant/tool result is illegal.
            hooks.continued()?;
            Ok(WorkerOutput {
                usage: None,
                text: "unreachable".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    });
    let (events, mut observed) = mpsc::channel(1);
    let (_commands, commands) = mpsc::channel(1);
    let (_cancel, cancelled) = oneshot::channel();
    let count = Arc::new(AtomicUsize::new(0));
    let sent = count.clone();
    let result = worker::run(
        1,
        json!({"tools":[{}]}),
        Some(handler),
        Instant::now() + Duration::from_secs(5),
        Box::new(|| Box::pin(async { Ok(()) })),
        Box::new(move || {
            sent.fetch_add(1, Ordering::SeqCst);
        }),
        cancelled,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
        Some(tools::Bridge { events, commands }),
    )
    .await;
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("cursor_remote_worker_protocol")
    );
    let phase = observed.try_recv().unwrap().unwrap();
    assert_eq!(phase.call.unwrap().id, "c");
    assert_eq!(phase.thinking.unwrap().text(), "public");
    assert!(phase.publication.is_none());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(observed.try_recv().is_err());
}
