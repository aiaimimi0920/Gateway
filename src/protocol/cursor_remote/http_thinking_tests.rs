//! Real router/SQLite/in-process worker, with synthetic thinking and MCP data only.
use super::*;

fn thinking_tool_worker() -> Runtime {
    Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            if input.get("thinking").is_some()
                || input["prompt"].as_str().is_some_and(|p| p.contains("公开"))
            {
                return Err(super::super::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            let patch_args = json!({"input":"patch"}).to_string();
            for (call_id, name, args, text, thinking) in [
                ("call_a", "echo", "{}", "prelude", " \r\n公开 A\t"),
                ("call_b", "edit", patch_args.as_str(), "", "公开 B"),
            ] {
                let result = hooks
                    .tool(WorkerToolCall {
                        call_id: call_id.into(),
                        name: name.into(),
                        arguments: args.into(),
                        text: text.into(),
                        thinking: Some(thinking.into()),
                        requested_model: "default".into(),
                    })
                    .await?;
                if result.output != "ok" {
                    return Err(super::super::error("worker_failed"));
                }
                hooks.continued()?;
            }
            Ok(WorkerOutput {
                usage: None,
                text: "answer".into(),
                thinking: Some("公开 C".into()),
                requested_model: "default".into(),
            })
        })
    })
}

fn request(history: &[Value], chat: bool, stream: bool) -> Value {
    if chat {
        super::chat::body(json!(history), stream)
    } else {
        json!({"model":"cursor-personal","store":false,"stream":stream,"input":history,
            "tools":[{"type":"function","name":"echo","parameters":{"type":"object"}},
                {"type":"custom","name":"edit","format":{"type":"text"}}]})
    }
}
fn response(raw: &str, chat: bool, stream: bool) -> Value {
    if chat {
        super::chat::message(raw, stream)
    } else if stream {
        events(raw)
            .into_iter()
            .find(|e| e["type"] == "response.completed")
            .unwrap()["response"]
            .clone()
    } else {
        serde_json::from_str(raw).unwrap()
    }
}
fn events(raw: &str) -> Vec<Value> {
    raw.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}
fn check(value: &Value, expected: &str, chat: bool) {
    if chat {
        assert_eq!(value["reasoning_content"], expected);
        assert!(!value["content"].as_str().unwrap_or("").contains("公开"));
    } else {
        let item = &value["output"][0];
        assert_eq!(item["type"], "reasoning");
        assert_eq!(item["summary"], json!([]));
        assert!(item.get("encrypted_content").is_none());
        assert_eq!(item["status"], "completed");
        assert_eq!(
            item["content"],
            json!([{"type":"reasoning_text","text":expected}])
        );
        assert!(!value["output"].as_array().unwrap()[1..]
            .iter()
            .any(|v| v.to_string().contains("公开")));
    }
}
fn append(history: &mut Vec<Value>, value: Value, chat: bool) {
    if chat {
        let id = value["tool_calls"][0]["id"].clone();
        history.push(value);
        history.push(json!({"role":"tool","tool_call_id":id,"content":"ok"}));
    } else {
        let call = value["output"].as_array().unwrap().last().unwrap();
        let result = json!({"type":if call["type"] == "custom_tool_call" {"custom_tool_call_output"} else {"function_call_output"},
            "call_id":call["call_id"],"output":"ok"});
        history.extend(value["output"].as_array().unwrap().clone());
        history.push(result);
    }
}
fn check_events(raw: &str, value: &Value, expected: &str) {
    let events = events(raw);
    let item = &value["output"][0];
    let reasoning: Vec<_> = events.iter().filter(|e| e["output_index"] == 0).collect();
    assert_eq!(
        reasoning
            .iter()
            .map(|v| v["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "response.output_item.added",
            "response.content_part.added",
            "response.reasoning_text.delta",
            "response.reasoning_text.done",
            "response.content_part.done",
            "response.output_item.done"
        ]
    );
    assert_eq!(reasoning[0]["item"]["content"], json!([]));
    assert_eq!(reasoning[2]["delta"], expected);
    assert_eq!(reasoning[3]["text"], expected);
    assert_eq!(reasoning[5]["item"], *item);
    for event in &reasoning[1..5] {
        assert_eq!(event["item_id"], item["id"]);
        assert_eq!(event["content_index"], 0);
    }
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["sequence_number"].as_u64(), Some(index as u64 + 1));
    }
    let done = events
        .iter()
        .filter(|v| v["type"] == "response.output_item.done")
        .map(|event| event["item"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        done.as_slice(),
        value["output"].as_array().unwrap().as_slice()
    );
}

#[test]
fn cursor_public_thinking_http_json_sse_echo_is_bound_and_never_sent_back_to_worker() {
    run(|| async {
        let state = app_worker(true, thinking_tool_worker()).await;
        let token = key(&state, "relay", "cursor-remote").await;
        for chat in [false, true] {
            let endpoint = if chat {
                "/v1/chat/completions"
            } else {
                "/v1/responses"
            };
            for stream in [false, true] {
                let mut history = vec![json!({"role":"user","content":"hello"})];
                for (index, expected) in [" \r\n公开 A\t", "公开 B", "公开 C"].iter().enumerate()
                {
                    let (status, raw) =
                        post_to(&state, &token, endpoint, request(&history, chat, stream)).await;
                    assert_eq!(status, StatusCode::OK, "{raw}");
                    let value = response(&raw, chat, stream);
                    check(&value, expected, chat);
                    if stream && !chat {
                        check_events(&raw, &value, expected);
                    }
                    if index == 2 {
                        assert!(raw.contains("answer"));
                        assert!(!raw.contains("公开 A") && !raw.contains("公开 B"));
                        break;
                    }
                    append(&mut history, value, chat);
                    if !chat && stream {
                        // Actual codex-cli 0.155.1 input serialization, not an
                        // unchanged copy of the Gateway's output-only fields.
                        for item in &mut history {
                            if item["type"] == "reasoning" {
                                item.as_object_mut().unwrap().remove("status");
                                item["encrypted_content"] = Value::Null;
                            }
                        }
                        for (field, replacement) in [
                            ("encrypted_content", json!("ciphertext")),
                            ("encrypted_content", json!("")),
                            ("status", Value::Null),
                            ("status", json!("in_progress")),
                        ] {
                            let mut bad = history.clone();
                            bad[1][field] = replacement;
                            assert_eq!(
                                post_to(&state, &token, endpoint, request(&bad, chat, stream))
                                    .await
                                    .0,
                                StatusCode::BAD_REQUEST
                            );
                        }
                    }
                    let mut bad = history.clone();
                    if chat {
                        bad[1]["reasoning_content"] = json!("公开 A");
                    } else {
                        bad[1]["content"][0]["text"] = json!("公开 A");
                    }
                    assert_ne!(
                        post_to(&state, &token, endpoint, request(&bad, chat, stream))
                            .await
                            .0,
                        StatusCode::OK
                    );
                    if !chat {
                        let mut bad = history.clone();
                        bad[1]["id"] = json!("rs_cursor_other");
                        assert_ne!(
                            post_to(&state, &token, endpoint, request(&bad, chat, stream))
                                .await
                                .0,
                            StatusCode::OK
                        );
                        let mut bad = history.clone();
                        bad.swap(1, 2);
                        assert_ne!(
                            post_to(&state, &token, endpoint, request(&bad, chat, stream))
                                .await
                                .0,
                            StatusCode::OK
                        );
                    }
                    let mut missing = history.clone();
                    if chat {
                        missing[1]
                            .as_object_mut()
                            .unwrap()
                            .remove("reasoning_content");
                    } else {
                        missing.remove(1);
                    }
                    assert_ne!(
                        post_to(&state, &token, endpoint, request(&missing, chat, stream))
                            .await
                            .0,
                        StatusCode::OK
                    );
                }
                state.lifecycle.cursor_remote.fixture_wait_idle().await;
            }
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_public_thinking_text_lane_and_failed_worker_cannot_fake_http_success() {
    run(|| async {
        for broken in [false, true] {
            let runtime = Runtime::fixture(move |_input, hooks| {
                Box::pin(async move {
                    hooks.authorize().await?;
                    hooks.sent()?;
                    if broken {
                        // Empty final answer with thinking-only output must be rejected.
                        Ok(WorkerOutput {
                            usage: None,
                            text: String::new(),
                            thinking: Some("public".into()),
                            requested_model: "default".into(),
                        })
                    } else {
                        Ok(WorkerOutput {
                            usage: None,
                            text: "answer".into(),
                            thinking: Some("公开 C".into()),
                            requested_model: "default".into(),
                        })
                    }
                })
            });
            let state = app_worker(true, runtime).await;
            let token = key(&state, "relay", "cursor-remote").await;
            for chat in [false, true] {
                for stream in [false, true] {
                    let mut body =
                        request(&[json!({"role":"user","content":"hello"})], chat, stream);
                    body.as_object_mut().unwrap().remove("tools");
                    let endpoint = if chat {
                        "/v1/chat/completions"
                    } else {
                        "/v1/responses"
                    };
                    let (status, raw) = post_to(&state, &token, endpoint, body).await;
                    if broken {
                        assert_ne!(status, StatusCode::OK);
                        assert!(
                            !raw.contains("response.completed")
                                && !raw.contains("\"reasoning_content\"")
                        );
                    } else {
                        assert_eq!(status, StatusCode::OK, "{raw}");
                        check(&response(&raw, chat, stream), "公开 C", chat);
                    }
                }
            }
            state.lifecycle.cursor_remote.join().await;
            state.local_runtime.as_ref().unwrap().close().await;
        }
    });
}
