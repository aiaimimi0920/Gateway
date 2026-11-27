use super::{
    worker::{WorkerOutput, WorkerToolCall},
    *,
};
use crate::protocol::cursor_remote::tool_request::{parse, ToolRequest};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::time::{timeout, Duration, Instant};

pub(super) fn fixture_runtime() -> Runtime {
    Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            let output = hooks
                .tool(WorkerToolCall {
                    call_id: "call_one".into(),
                    name: "echo".into(),
                    arguments: r#"{"value":"hello"}"#.into(),
                    text: "prelude".into(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if output.call_id != "call_one" || output.output != "tool output" {
                return Err(crate::protocol::cursor_remote::error("worker_failed"));
            }
            hooks.continued()?;
            Ok(WorkerOutput {
                usage: None,
                text: "final answer".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

pub(super) fn request(input: serde_json::Value) -> ToolRequest {
    parse(
        &crate::protocol::responses::normalize_responses(json!({"model":"cursor-personal",
        "store":false,"input":input,"tools":[{"type":"function","name":"echo","strict":false,
        "parameters":{"type":"object","properties":{"value":{"type":"string"}}}}]}))
        .unwrap(),
    )
    .unwrap()
}

pub(super) fn resume() -> ToolRequest {
    request(
        json!([{"role":"user","content":"hello"},{"role":"assistant","content":"prelude"},
        {"type":"function_call","call_id":"call_one","name":"echo","arguments":"{\"value\":\"hello\"}"},
        {"type":"function_call_output","call_id":"call_one","output":"tool output"}]),
    )
}

pub(super) async fn phase(
    rt: &Runtime,
    request: ToolRequest,
    binding: &str,
    denied: bool,
    count: Arc<AtomicUsize>,
) -> Result<Completed, GatewayError> {
    let (settled, receiver) = tokio::sync::watch::channel(false);
    let mut result = rt
        .execute_tools(
            binding.into(),
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
                count.fetch_add(1, Ordering::SeqCst);
            }),
            settled,
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    if let Ok(completed) = &mut result {
        if let Some(publication) = &mut completed.publication {
            publication.commit();
        }
    }
    timeout(Duration::from_secs(5), settle(Some(&receiver)))
        .await
        .unwrap();
    result
}

#[tokio::test]
async fn cursor_remote_tools_one_run_two_http_phases_exact_history_and_single_consumption() {
    let rt = fixture_runtime();
    let count = Arc::new(AtomicUsize::new(0));
    let first = phase(
        &rt,
        request(json!("hello")),
        "key/credential/revision",
        false,
        count.clone(),
    )
    .await
    .unwrap();
    assert_eq!(first.call.unwrap().id, "call_one");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    for binding in [
        "different-key/credential/revision",
        "key/other-account/revision",
        "key/credential/other-revision",
    ] {
        assert!(phase(&rt, resume(), binding, false, count.clone())
            .await
            .is_err());
    }
    let mut bad = resume();
    bad.history[0]["content"] = json!("tampered");
    assert!(
        phase(&rt, bad, "key/credential/revision", false, count.clone())
            .await
            .is_err()
    );
    let reply = phase(
        &rt,
        resume(),
        "key/credential/revision",
        false,
        count.clone(),
    )
    .await
    .unwrap();
    assert_eq!(reply.text, "final answer");
    assert!(reply.call.is_none());
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(
        phase(&rt, resume(), "key/credential/revision", false, count)
            .await
            .is_err()
    );
    timeout(Duration::from_secs(5), rt.join()).await.unwrap();
}

#[tokio::test]
async fn cursor_remote_tools_revoked_resume_reaps_without_result_send() {
    let rt = fixture_runtime();
    let count = Arc::new(AtomicUsize::new(0));
    phase(&rt, request(json!("hello")), "owner", false, count.clone())
        .await
        .unwrap();
    assert!(phase(&rt, resume(), "owner", true, count.clone())
        .await
        .is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    timeout(Duration::from_secs(5), rt.join()).await.unwrap();
}

#[tokio::test]
async fn cursor_remote_tools_drain_cancels_parked_worker_and_releases_slot() {
    let rt = fixture_runtime();
    phase(
        &rt,
        request(json!("hello")),
        "owner",
        false,
        Arc::new(AtomicUsize::new(0)),
    )
    .await
    .unwrap();
    timeout(Duration::from_secs(5), rt.join()).await.unwrap();
    assert!(!rt.available());
}

#[test]
fn cursor_remote_tools_do_not_admit_malformed_custom_strict_or_new_run_replay() {
    let base = json!({"model":"cursor-personal","input":"hello","store":false,
        "tools":[{"type":"function","name":"echo","parameters":{"type":"object"}}]});
    for (pointer, value) in [
        ("/tools/0/type", json!("custom")),
        ("/tools/0/strict", json!(true)),
        ("/tool_choice", json!("required")),
        ("/store", json!(true)),
    ] {
        let mut value_body = base.clone();
        if let Some(slot) = value_body.pointer_mut(pointer) {
            *slot = value;
        } else if pointer == "/tools/0/strict" {
            value_body["tools"][0]["strict"] = value;
        } else {
            value_body["tool_choice"] = value;
        }
        assert!(
            parse(&crate::protocol::responses::normalize_responses(value_body).unwrap()).is_err()
        );
    }
    let mut replay = resume();
    replay
        .history
        .push(json!({"role":"user","content":"new question"}));
    let mut body = base;
    body["input"] = json!(replay.history);
    assert!(parse(&crate::protocol::responses::normalize_responses(body).unwrap()).is_err());
}
