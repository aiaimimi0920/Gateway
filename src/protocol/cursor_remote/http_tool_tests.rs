use super::*;

fn exec_tool_worker() -> Runtime {
    Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            if input["tools"][0]["name"] != "exec_command" {
                return Err(super::super::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            let output = hooks
                .tool(WorkerToolCall {
                    call_id: "call_exec".into(),
                    name: "exec_command".into(),
                    arguments: r#"{"cmd":"node --test"}"#.into(),
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if output.output != "1 passed" {
                return Err(super::super::error("worker_failed"));
            }
            hooks.continued()?;
            Ok(WorkerOutput {
                usage: None,
                text: "All tests passed".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

fn body(input: Value, stream: bool) -> Value {
    json!({"model":"cursor-personal","input":input,"store":false,"stream":stream,"tools":[{
        "type":"function","name":"exec_command","strict":false,"description":"Run a local command",
        "parameters":{"type":"object","properties":{"cmd":{"type":"string"}},"required":["cmd"]}
    }]})
}

#[test]
fn cursor_remote_http_dropped_tool_body_reaps_run_before_next_request() {
    run(|| async {
        use futures::StreamExt;
        let state = app_worker(true, exec_tool_worker()).await;
        let token = key(&state, "responses", "cursor-remote").await;
        for partial in [false, true] {
            for stream in [false, true] {
                if partial && !stream {
                    continue;
                }
                let request = Request::post("/v1/responses")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body(json!("hello"), stream).to_string()))
                    .unwrap();
                let response = crate::http::router::build_router(state.clone())
                    .oneshot(request)
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                let mut bytes = response.into_body().into_data_stream();
                if partial {
                    assert!(bytes.next().await.unwrap().is_ok());
                }
                drop(bytes);
                tokio::time::timeout(
                    Duration::from_secs(5),
                    state.lifecycle.cursor_remote.fixture_wait_idle(),
                )
                .await
                .unwrap();
            }
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

fn response(raw: &str, stream: bool) -> Value {
    if !stream {
        return serde_json::from_str(raw).unwrap();
    }
    raw.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|event| event["type"] == "response.completed")
        .unwrap()["response"]
        .clone()
}

#[test]
fn cursor_remote_http_native_function_cycle_json_sse_and_wrong_key_cannot_claim() {
    run(|| async {
        let state = app_worker(true, exec_tool_worker()).await;
        let token = key(&state, "responses", "cursor-remote").await;
        let other = key(&state, "responses", "cursor-remote").await;
        for stream in [false, true] {
            let (status, raw) = post(&state, &token, body(json!("hello"), stream)).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            if stream {
                for event in [
                    "response.created",
                    "response.output_item.added",
                    "response.function_call_arguments.delta",
                    "response.output_item.done",
                    "response.completed",
                ] {
                    assert!(raw.contains(event), "{event}: {raw}");
                }
            }
            let first = response(&raw, stream);
            let call = first["output"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["type"] == "function_call")
                .unwrap();
            assert_eq!(call["call_id"], "call_exec");
            assert_eq!(call["arguments"], "{\"cmd\":\"node --test\"}");
            let mut items = vec![json!({"role":"user","content":"hello"})];
            items.extend(first["output"].as_array().unwrap().clone());
            items.push(
                json!({"type":"function_call_output","id":"fco_codex_client","call_id":"call_exec",
                    "output":[{"type":"input_text","text":"1 "},{"type":"input_text","text":"passed"}]}),
            );
            let resumed = body(json!(items), stream);
            let (status, _) = post(&state, &other, resumed.clone()).await;
            assert_ne!(status, StatusCode::OK);
            let (status, raw) = post(&state, &token, resumed.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            assert!(raw.contains("All tests passed"), "{raw}");
            let (status, _) = post(&state, &token, resumed).await;
            assert_ne!(status, StatusCode::OK);
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
