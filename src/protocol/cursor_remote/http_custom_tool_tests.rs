//! Real router/SQLite/pure-Rust worker fixture; the synthetic MCP envelope is not upstream evidence.
use super::*;

const INPUT: &str = " \r\n补丁\t\"<&amp;>\"\n ";

fn body(input: Value, stream: bool) -> Value {
    json!({"model":"cursor-personal","input":input,"store":false,"stream":stream,
        "tools":[{"type":"custom","name":"edit","format":{"type":"text"}}]})
}

fn custom_worker(expected_input: &str) -> Runtime {
    let expected = expected_input.to_owned();
    Runtime::fixture(move |input, hooks| {
        let expected = expected.clone();
        Box::pin(async move {
            let t = &input["tools"][0];
            if t["name"] != "edit" || t["parameters"]["properties"]["input"]["type"] != "string" {
                return Err(super::super::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            let output = hooks
                .tool(WorkerToolCall {
                    call_id: "call_edit".into(),
                    name: "edit".into(),
                    arguments: json!({"input": expected}).to_string(),
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if output.output != "edited" {
                return Err(super::super::error("worker_failed"));
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

fn events(raw: &str) -> Vec<Value> {
    raw.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str(data).ok())
        .collect()
}

fn response(raw: &str, stream: bool) -> Value {
    if stream {
        events(raw)
            .into_iter()
            .find(|e| e["type"] == "response.completed")
            .unwrap()["response"]
            .clone()
    } else {
        serde_json::from_str(raw).unwrap()
    }
}

#[test]
fn cursor_remote_http_custom_text_json_sse_preserves_call_and_rejects_cross_type_results() {
    run(|| async {
        for input in ["", INPUT] {
            let state = app_worker(true, custom_worker(input)).await;
            let token = key(&state, "responses", "cursor-remote").await;
            for stream in [false, true] {
                let (status, raw) = post(&state, &token, body(json!("hello"), stream)).await;
                assert_eq!(status, StatusCode::OK, "{raw}");
                let first = response(&raw, stream);
                let call = first["output"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|item| item["type"] == "custom_tool_call")
                    .unwrap();
                assert_eq!(call["call_id"], "call_edit");
                assert_eq!(call["name"], "edit");
                assert_eq!(call["input"], input);
                assert!(call.get("arguments").is_none());
                if stream {
                    let events = events(&raw);
                    let tool_events: Vec<&str> = events
                        .iter()
                        .filter_map(|e| e["type"].as_str())
                        .filter(|kind| {
                            kind.contains("output_item") || kind.contains("tool_call_input")
                        })
                        .collect();
                    let mut expected = vec!["response.output_item.added"];
                    if !input.is_empty() {
                        expected.push("response.custom_tool_call_input.delta");
                    }
                    expected.extend([
                        "response.custom_tool_call_input.done",
                        "response.output_item.done",
                    ]);
                    assert_eq!(tool_events, expected);
                    assert!(!raw.contains("response.function_call_arguments"));
                    let delta = events
                        .iter()
                        .find(|e| e["type"] == "response.custom_tool_call_input.delta");
                    let done = events
                        .iter()
                        .find(|e| e["type"] == "response.custom_tool_call_input.done")
                        .unwrap();
                    assert_eq!(done["input"], input);
                    assert_eq!(done["item_id"], call["id"]);
                    if input.is_empty() {
                        assert!(delta.is_none());
                    } else {
                        let delta = delta.unwrap();
                        assert_eq!(delta["delta"], input);
                        assert_eq!(delta["item_id"], call["id"]);
                        assert_eq!(delta["output_index"], done["output_index"]);
                    }
                }
                let mut history = vec![json!({"role":"user","content":"hello"})];
                history.extend(first["output"].as_array().unwrap().clone());
                history.push(
                    json!({"type":"custom_tool_call_output","id":"not_the_call_id",
                    "call_id":"call_edit","output":[{"type":"input_text","text":"ed"},{"type":"input_text","text":"ited"}]}),
                );
                let mut malformed = history.clone();
                malformed[0]["type"] = Value::Null;
                let (status, rejected) = post(&state, &token, body(json!(malformed), stream)).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
                assert!(rejected.contains("cursor_remote_unsupported_tool_request"));
                let mut bad = history.clone();
                bad.last_mut().unwrap()["type"] = json!("function_call_output");
                assert_ne!(
                    post(&state, &token, body(json!(bad), stream)).await.0,
                    StatusCode::OK
                );
                let resumed = body(json!(history), stream);
                let (status, final_body) = post(&state, &token, resumed.clone()).await;
                assert_eq!(status, StatusCode::OK, "{final_body}");
                assert_eq!(response(&final_body, stream)["status"], "completed");
                assert!(final_body.contains("done"));
                assert_ne!(post(&state, &token, resumed).await.0, StatusCode::OK);
            }
            state.lifecycle.cursor_remote.join().await;
            state.local_runtime.as_ref().unwrap().close().await;
        }
    });
}
