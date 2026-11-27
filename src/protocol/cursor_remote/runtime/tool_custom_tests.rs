//! Custom text uses the same one-shot live owner, without changing native IPC.
use super::{
    tool_tests::phase,
    worker::{WorkerOutput, WorkerToolCall},
    *,
};
use crate::protocol::cursor_remote::{
    tool_request::{parse, ToolRequest},
    tool_schema::ToolKind,
};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

const INPUT: &str = " \r\n补丁\t\"<&amp;>\"\n ";
fn request(input: Value) -> ToolRequest {
    parse(
        &crate::protocol::responses::normalize_responses(json!({
            "model":"cursor-personal","store":false,"input":input,
            "tools":[{"type":"custom","name":"edit","format":{"type":"text"}}],
        }))
        .unwrap(),
    )
    .unwrap()
}
fn resume() -> ToolRequest {
    request(json!([
        {"role":"user","content":"hello"},
        {"type":"custom_tool_call","call_id":"call_edit","name":"edit","input":INPUT},
        {"type":"custom_tool_call_output","call_id":"call_edit","output":"edited"},
    ]))
}
fn custom_runtime(arguments: &str) -> Runtime {
    let args = arguments.to_owned();
    Runtime::fixture(move |input, hooks| {
        let args = args.clone();
        Box::pin(async move {
            let schema = &input["tools"][0]["parameters"];
            if schema["properties"]["input"]["type"] != "string"
                || schema["additionalProperties"] != false
            {
                return Err(crate::protocol::cursor_remote::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            let output = hooks
                .tool(WorkerToolCall {
                    call_id: "call_edit".into(),
                    name: "edit".into(),
                    arguments: args,
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if output.call_id != "call_edit" || output.output != "edited" {
                return Err(crate::protocol::cursor_remote::error("worker_failed"));
            }
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

#[tokio::test]
async fn cursor_remote_custom_one_run_preserves_text_and_rejects_wrong_output_kind() {
    let rt = custom_runtime(&json!({"input":INPUT}).to_string());
    let count = Arc::new(AtomicUsize::new(0));
    let first = phase(&rt, request(json!("hello")), "owner", false, count.clone())
        .await
        .unwrap();
    let call = first.call.unwrap();
    assert_eq!(call.kind, ToolKind::CustomText);
    assert_eq!(call.arguments, INPUT);
    assert_eq!(call.history_item()["type"], "custom_tool_call");
    let canonical = call.into_canonical();
    assert_eq!(canonical.call_type, "custom");
    assert_eq!(canonical.arguments.as_deref(), Some(INPUT));
    for kind in ["wrong-key", "wrong-kind", "wrong-id", "wrong-history"] {
        let mut bad = resume();
        let binding = if kind == "wrong-key" {
            "other"
        } else {
            "owner"
        };
        match kind {
            "wrong-kind" => bad.output.as_mut().unwrap().kind = ToolKind::Function,
            "wrong-id" => bad.output.as_mut().unwrap().call_id = "item_not_call".into(),
            "wrong-history" => bad.history[1]["input"] = json!(INPUT.trim()),
            _ => {}
        }
        assert!(phase(&rt, bad, binding, false, count.clone())
            .await
            .is_err());
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let result = phase(&rt, resume(), "owner", false, count.clone())
        .await
        .unwrap();
    assert_eq!(result.text, "done");
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert!(phase(&rt, resume(), "owner", false, count.clone())
        .await
        .is_err());
    rt.join().await;
}

#[tokio::test]
async fn cursor_remote_invalid_custom_native_envelope_cancels_without_publication() {
    for arguments in [
        "{\"input\":5}",
        "{\"arguments\":\"text\"}",
        "{\"input\":\"text\",\"extra\":true}",
    ] {
        let rt = custom_runtime(arguments);
        let count = Arc::new(AtomicUsize::new(0));
        let error = phase(&rt, request(json!("hello")), "owner", false, count.clone())
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("cursor_remote_invalid_custom_tool_input")
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
        tokio::time::timeout(std::time::Duration::from_secs(5), rt.join())
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn cursor_remote_mixed_tools_keep_each_kind_and_empty_text_on_one_run() {
    let rt = Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            let arr = input["tools"].as_array().unwrap();
            if arr.len() != 2 || arr[1]["parameters"]["properties"]["input"]["type"] != "string" {
                return Err(crate::protocol::cursor_remote::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            for (i, name, args, expected) in [
                (0usize, "echo", r#"{"input":"still a function"}"#, "echoed"),
                (1, "edit", r#"{"input":""}"#, ""),
                (2, "echo", "{}", "finished"),
            ] {
                let id = format!("call_{i}");
                let output = hooks
                    .tool(WorkerToolCall {
                        call_id: id.clone(),
                        name: name.into(),
                        arguments: args.into(),
                        text: String::new(),
                        thinking: None,
                        requested_model: "default".into(),
                    })
                    .await?;
                if output.call_id != id || output.output != expected {
                    return Err(crate::protocol::cursor_remote::error("worker_failed"));
                }
                hooks.continued()?;
            }
            Ok(WorkerOutput {
                usage: None,
                text: "done".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    });
    let tools = json!([
        {"type":"function","name":"echo","parameters":{"type":"object"}},
        {"type":"custom","name":"edit"},
    ]);
    let mut history = vec![json!({"role":"user","content":"hello"})];
    let parse_history = |history: &[Value]| {
        parse(
            &crate::protocol::responses::normalize_responses(json!({
                "model":"cursor-personal","store":false,"input":history,"tools":tools,
            }))
            .unwrap(),
        )
        .unwrap()
    };
    let count = Arc::new(AtomicUsize::new(0));
    for (index, (kind, name, input, output)) in [
        (
            ToolKind::Function,
            "echo",
            "{\"input\":\"still a function\"}",
            "echoed",
        ),
        (ToolKind::CustomText, "edit", "", ""),
        (ToolKind::Function, "echo", "{}", "finished"),
    ]
    .into_iter()
    .enumerate()
    {
        let result = phase(&rt, parse_history(&history), "owner", false, count.clone())
            .await
            .unwrap();
        let call = result.call.unwrap();
        assert_eq!(call.id, format!("call_{index}"));
        assert_eq!(call.name, name);
        assert_eq!(call.kind, kind);
        assert_eq!(call.arguments, input);
        assert_eq!(count.load(Ordering::SeqCst), index + 1);
        history.push(call.history_item());
        history.push(json!({"type":kind.output_type(),"call_id":call.id,"output":output}));
    }
    let result = phase(&rt, parse_history(&history), "owner", false, count.clone())
        .await
        .unwrap();
    assert!(result.call.is_none());
    assert_eq!(result.text, "done");
    assert_eq!(count.load(Ordering::SeqCst), 4);
    rt.join().await;
}
