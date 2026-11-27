//! Real router/SQLite and synthetic in-process Rust worker; not a Cursor upstream acceptance.
use super::*;

pub(super) fn chat_worker() -> Runtime {
    Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            if input["tools"].as_array().map(Vec::len) != Some(2) {
                return Err(super::super::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            let first = hooks
                .tool(WorkerToolCall {
                    call_id: "call_echo".into(),
                    name: "echo".into(),
                    arguments: r#"{"text":"&quot;中文"}"#.into(),
                    text: "prelude".into(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if first.output != " \r\n函数" {
                return Err(super::super::error("worker_failed"));
            }
            hooks.continued()?;
            let second = hooks
                .tool(WorkerToolCall {
                    call_id: "call_edit".into(),
                    name: "edit".into(),
                    arguments: json!({"input":" \r\n补丁&amp;\t"}).to_string(),
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if second.output != "edited" {
                return Err(super::super::error("worker_failed"));
            }
            hooks.continued()?;
            Ok(WorkerOutput {
                usage: None,
                text: "finished coding".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

pub(super) fn body(messages: Value, stream: bool) -> Value {
    json!({"model":"cursor-personal","messages":messages,"stream":stream,
        "tools":[{"type":"function","function":{"name":"echo","parameters":{"type":"object"}}},
            {"type":"custom","custom":{"name":"edit","format":{"type":"text"}}}]})
}

async fn chat(state: &Arc<AppState>, token: &str, body: Value) -> (StatusCode, String) {
    post_to(state, token, "/v1/chat/completions", body).await
}

#[test]
fn cursor_chat_diagnostic_distinguishes_request_options_from_tool_errors() {
    run(|| async {
        let state = app(true).await;
        let token = key(&state, "chat.completions", "cursor-remote").await;
        let body =
            json!({"model":"cursor-personal","messages":[{"role":"user","content":"hello"}]});
        for (field, value, code) in [
            ("n", json!(2), "cursor_remote_unsupported_request"),
            ("max_tokens", json!(16), "cursor_remote_unsupported_request"),
            (
                "tools",
                json!([{"type":"unknown"}]),
                "cursor_remote_unsupported_tool_request",
            ),
        ] {
            let mut req = body.clone();
            req[field] = value;
            let (status, raw) = chat(&state, &token, req).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {raw}");
            assert_eq!(
                serde_json::from_str::<Value>(&raw).unwrap()["error"]["code"],
                code
            );
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

pub(super) fn message(raw: &str, stream: bool) -> Value {
    if !stream {
        let value: Value = serde_json::from_str(raw).unwrap();
        assert!(value.get("usage").is_none());
        assert_eq!(value["object"], "chat.completion");
        return value["choices"][0]["message"].clone();
    }
    assert!(raw.ends_with("data: [DONE]\n\n"));
    let mut message = json!({"content":""});
    let mut content = String::new();
    for line in raw.lines().filter_map(|l| l.strip_prefix("data: ")) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        assert_eq!(value["object"], "chat.completion.chunk");
        assert!(value.get("usage").is_none());
        let delta = &value["choices"][0]["delta"];
        if let Some(role) = delta.get("role") {
            message["role"] = role.clone();
        }
        if let Some(text) = delta["content"].as_str() {
            content.push_str(text);
        }
        if let Some(text) = delta["reasoning_content"].as_str() {
            let previous = message["reasoning_content"].as_str().unwrap_or("");
            message["reasoning_content"] = json!(format!("{previous}{text}"));
        }
        if let Some(calls) = delta.get("tool_calls") {
            let mut calls = calls.clone();
            for call in calls.as_array_mut().unwrap() {
                call.as_object_mut().unwrap().remove("index");
            }
            message["tool_calls"] = calls;
        }
    }
    message["content"] = json!(content);
    assert_eq!(message["role"], "assistant");
    message
}

#[test]
fn cursor_remote_chat_text_json_sse_and_scope_isolation() {
    run(|| async {
        let state = app(true).await;
        let chat_key = key(&state, "chat.completions", "cursor-remote").await;
        let responses_key = key(&state, "responses", "cursor-remote").await;
        for stream in [false, true] {
            let body = json!({"model":"cursor-personal","stream":stream,
                "messages":[{"role":"user","content":"hello"}]});
            let (status, raw) = chat(&state, &chat_key, body.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            assert_eq!(message(&raw, stream)["content"], "remote fixture");
            assert_ne!(chat(&state, &responses_key, body).await.0, StatusCode::OK);
            assert_ne!(
                post(
                    &state,
                    &chat_key,
                    json!({"model":"cursor-personal","input":"hello","stream":stream})
                )
                .await
                .0,
                StatusCode::OK
            );
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_chat_function_custom_same_run_preserves_history_and_protocol_binding() {
    run(|| async {
        let state = app_worker(true, chat_worker()).await;
        let token = key(&state, "relay", "cursor-remote").await;
        let other = key(&state, "relay", "cursor-remote").await;
        for stream in [false, true] {
            let mut history = vec![json!({"role":"user","content":"hello"})];
            let (status, raw) = chat(&state, &token, body(json!(history), stream)).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            let first = message(&raw, stream);
            assert_eq!(
                first["tool_calls"][0]["function"]["arguments"],
                "{\"text\":\"&quot;中文\"}"
            );
            history.push(first);
            history.push(json!({"role":"tool","tool_call_id":"call_echo","content":[
                {"type":"text","text":" \r\n"},{"type":"text","text":"函数"}]}));
            let mut cross = body(json!(history), stream);
            let req = crate::protocol::openai::normalize_chat_completions(cross.clone()).unwrap();
            cross = crate::protocol::cursor_remote::chat_request::project(&req)
                .unwrap()
                .raw_body;
            let (status, rejected) = post(&state, &token, cross).await;
            assert_ne!(status, StatusCode::OK);
            assert!(
                rejected.contains("cursor_remote_continuation_mismatch"),
                "{rejected}"
            );
            assert_ne!(
                chat(&state, &other, body(json!(history), stream)).await.0,
                StatusCode::OK
            );
            let mut changed = body(json!(history), stream);
            changed["tool_choice"] = json!("none");
            assert_eq!(
                chat(&state, &token, changed).await.0,
                StatusCode::BAD_REQUEST
            );
            let (status, raw) = chat(&state, &token, body(json!(history), stream)).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            let second = message(&raw, stream);
            assert_eq!(second["tool_calls"][0]["type"], "custom");
            assert_eq!(
                second["tool_calls"][0]["custom"]["input"],
                " \r\n补丁&amp;\t"
            );
            history.push(second);
            history.push(json!({"role":"tool","tool_call_id":"call_edit","content":"edited"}));
            history[2]["content"] = json!(" \r\n函数");
            let resumed = body(json!(history), stream);
            let (status, raw) = chat(&state, &token, resumed.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            assert_eq!(message(&raw, stream)["content"], "finished coding");
            assert_ne!(chat(&state, &token, resumed).await.0, StatusCode::OK);
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_none_never_sends_mcp_definitions_on_either_http_endpoint() {
    run(|| async {
        let state = app_worker(
            true,
            Runtime::fixture(|input, hooks| {
                Box::pin(async move {
                    if input.get("tools").is_some()
                        || !input["prompt"]
                            .as_str()
                            .is_some_and(|p| p.contains("do not call tools"))
                    {
                        return Err(super::super::error("worker_failed"));
                    }
                    hooks.authorize().await?;
                    hooks.sent()?;
                    Ok(WorkerOutput {
                        usage: None,
                        text: "no tool request".into(),
                        thinking: None,
                        requested_model: "default".into(),
                    })
                })
            }),
        )
        .await;
        let token = key(&state, "relay", "cursor-remote").await;
        for stream in [false, true] {
            let mut req = body(json!([{"role":"user","content":"hello"}]), stream);
            req["tool_choice"] = json!("none");
            let projected = crate::protocol::cursor_remote::chat_request::project(
                &crate::protocol::openai::normalize_chat_completions(req.clone()).unwrap(),
            )
            .unwrap();
            for (endpoint, body) in [
                ("/v1/chat/completions", req),
                ("/v1/responses", projected.raw_body),
            ] {
                let (status, raw) = post_to(&state, &token, endpoint, body).await;
                assert_eq!(status, StatusCode::OK, "{raw}");
                assert!(raw.contains("no tool request"));
            }
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_chat_dropped_body_reaps_unpublished_tool_phase() {
    run(|| async {
        let state = app_worker(true, chat_worker()).await;
        let token = key(&state, "chat.completions", "cursor-remote").await;
        for stream in [false, true] {
            let request = Request::post("/v1/chat/completions")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    body(json!([{"role":"user","content":"hello"}]), stream).to_string(),
                ))
                .unwrap();
            let response = crate::http::router::build_router(state.clone())
                .oneshot(request)
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            drop(response);
            tokio::time::timeout(
                Duration::from_secs(5),
                state.lifecycle.cursor_remote.fixture_wait_idle(),
            )
            .await
            .unwrap();
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
