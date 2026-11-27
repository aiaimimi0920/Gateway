//! Production body owner with an actual paused synthetic worker, no external calls.
use super::*;
use crate::protocol::cursor_remote::{
    runtime::{
        worker::{WorkerOutput, WorkerToolCall},
        Runtime,
    },
    tool_request,
};
use serde_json::json;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::time::{timeout, Instant};

fn paused_runtime() -> Runtime {
    Runtime::fixture(|_input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            let _ = hooks
                .tool(WorkerToolCall {
                    call_id: "call_x".into(),
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
    })
}
async fn paused(rt: &Runtime) -> Publication {
    let req = crate::protocol::responses::normalize_responses(json!({
        "model":"cursor-personal","input":"hello",
        "tools":[{"type":"function","name":"echo","parameters":{"type":"object"}}]
    }))
    .unwrap();
    rt.execute_tools(
        "owner".into(),
        tool_request::parse(&req).unwrap(),
        json!({}),
        Instant::now() + Duration::from_secs(5),
        Box::new(|| Box::pin(async { Ok(()) })),
        Box::new(|| {}),
        tokio::sync::watch::channel(false).0,
        Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap()
    .publication
    .unwrap()
}
fn events() -> Vec<Result<Bytes, io::Error>> {
    let call = json!({"id":"call_x","type":"function","function":{"name":"echo","arguments":"{}"}});
    [
        json!({"object":"chat.completion.chunk","choices":[{"index":0,
            "delta":{"tool_calls":[call]},"finish_reason":null}]})
        .to_string(),
        json!({"object":"chat.completion.chunk","choices":[{"index":0,
            "delta":{},"finish_reason":"tool_calls"}]})
        .to_string(),
        "[DONE]".into(),
    ]
    .into_iter()
    .map(|data| Ok(Bytes::from(format!("data: {data}\n\n"))))
    .collect()
}
async fn idle(rt: &Runtime) {
    timeout(Duration::from_secs(5), rt.fixture_wait_idle())
        .await
        .unwrap();
}

#[tokio::test]
async fn cursor_remote_chat_http_terminal_requires_clean_eof_and_live_guard() {
    for case in [
        "clean",
        "truncated",
        "suffix",
        "utf8",
        "error",
        "missing_done",
        "named_done",
        "oversize",
        "closed",
    ] {
        let rt = paused_runtime();
        let guard = paused(&rt).await;
        let mut frames = events();
        match case {
            "truncated" => frames.push(Ok(Bytes::from_static(b"data: {"))),
            "suffix" => frames.push(Ok(Bytes::from_static(b"data: {}\n\n"))),
            "utf8" => frames.push(Ok(Bytes::from_static(b"data: \xff\n\n"))),
            "error" => frames.push(Err(io::Error::other("fixture"))),
            "missing_done" => {
                frames.pop();
            }
            "named_done" => {
                *frames.last_mut().unwrap() =
                    Ok(Bytes::from_static(b"event: error\ndata: [DONE]\n\n"));
            }
            "oversize" => frames.push(Ok(Bytes::from(vec![b' '; LIMIT + 1]))),
            "closed" => rt.close(),
            _ => {}
        }
        let response = Response::builder()
            .header("content-type", "text/event-stream")
            .body(Body::from_stream(futures::stream::iter(frames)))
            .unwrap();
        let response = attach(response, guard);
        let mut body = response.into_body().into_data_stream();
        let result = timeout(Duration::from_secs(5), body.next())
            .await
            .unwrap()
            .unwrap();
        if case == "clean" {
            assert!(result.unwrap().ends_with(b"data: [DONE]\n\n"));
            assert!(body.next().await.is_none());
            // Successful publication leaves the Run available for a future HTTP request.
            assert!(timeout(Duration::from_millis(30), rt.fixture_wait_idle())
                .await
                .is_err());
        } else {
            assert!(result.is_err(), "{case}"); // no successful terminal leaked
            assert!(body.next().await.is_none());
        }
        drop(body);
        // Failure must reap by dropping the publication owner, not by this
        // fixture forcibly closing an otherwise leaked pause.
        if case == "clean" {
            rt.close();
        }
        idle(&rt).await;
        rt.join().await;
    }
}

#[tokio::test]
async fn cursor_remote_chat_partial_body_poll_still_owns_cancellation() {
    let rt = paused_runtime();
    let guard = paused(&rt).await;
    let body = Body::from_stream(
        futures::stream::iter(events().into_iter().take(1))
            .chain(futures::stream::pending::<Result<Bytes, io::Error>>()),
    );
    let response = Response::builder()
        .header("content-type", "text/event-stream")
        .body(body)
        .unwrap();
    let mut body = attach(response, guard).into_body().into_data_stream();
    assert!(timeout(Duration::from_millis(30), body.next())
        .await
        .is_err());
    drop(body);
    idle(&rt).await;
    rt.join().await;
}

#[test]
fn cursor_remote_chat_publication_rejects_false_tool_success() {
    let valid = json!({"object":"chat.completion","choices":[{"index":0,
    "finish_reason":"tool_calls","message":{"role":"assistant","tool_calls":[{
        "id":"call_x","type":"custom","custom":{"name":"edit","input":""}
    }]}}]});
    assert!(validate_json(valid.to_string().as_bytes()).is_ok());
    for (pointer, value) in [
        ("/choices/0/finish_reason", json!("stop")),
        ("/choices/0/message/tool_calls/0/type", json!("unknown")),
        ("/choices/0/message/tool_calls/0/id", json!("")),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(validate_json(bad.to_string().as_bytes()).is_err());
    }
}

#[tokio::test]
async fn cursor_remote_chat_invalid_json_body_cancels_without_publication() {
    let rt = paused_runtime();
    let guard = paused(&rt).await;
    let response = Response::builder()
        .header("content-type", "application/json")
        .body(Body::from(r#"{"object":"chat.completion","choices":[]}"#))
        .unwrap();
    let mut body = attach(response, guard).into_body().into_data_stream();
    assert!(body.next().await.unwrap().is_err());
    assert!(body.next().await.is_none());
    drop(body);
    idle(&rt).await;
    rt.join().await;
}
