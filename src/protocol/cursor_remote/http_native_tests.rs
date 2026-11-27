//! Real router/SQLite, synthetic native worker. These prove protocol/lifecycle
//! behavior only; live Cursor evidence is collected by the opt-in verifier.
use super::*;

fn worker(images: usize) -> Runtime {
    Runtime::fixture(move |input, hooks| {
        Box::pin(async move {
            assert_eq!(input["tools"].as_array().unwrap().len(), 1);
            assert_eq!(
                super::super::upstream::validate_images(input.get("images"))
                    .unwrap()
                    .len(),
                images
            );
            assert!(input["prompt"].as_str().unwrap().contains("hello"));
            hooks.authorize().await?;
            hooks.sent()?;
            for round in 0..2 {
                let result = hooks
                    .tool(WorkerToolCall {
                        call_id: format!("call_native_{round}"),
                        name: "echo".into(),
                        arguments: r#"{ "z":1, "q":{"b":2,"a":"中文🦀"}} "#.into(),
                        text: format!("prelude {round} 中文🦀"),
                        thinking: Some("public thinking without a native signed field".into()),
                        requested_model: "default".into(),
                    })
                    .await?;
                assert_eq!(result.output, json!({"result":round}).to_string());
                hooks.continued()?;
            }
            Ok(WorkerOutput {
                usage: None,
                text: "native complete 中文🦀".into(),
                thinking: Some("finished thinking".into()),
                requested_model: "default".into(),
            })
        })
    })
}

fn endpoint(gemini: bool, stream: bool) -> &'static str {
    match (gemini, stream) {
        (true, false) => "/v1beta/models/cursor-personal:generateContent",
        (true, true) => "/v1/models/cursor-personal:streamGenerateContent?alt=sse",
        _ => "/v2/chat",
    }
}

fn body(gemini: bool, stream: bool) -> Value {
    if gemini {
        json!({"contents":[{"role":"user","parts":[{"text":"hello"}]}],
            "tools":[{"functionDeclarations":[{"name":"echo","parametersJsonSchema":{"type":"object"}}]}]})
    } else {
        json!({"model":"cursor-personal","stream":stream,"messages":[{"role":"user","content":"hello"}],
            "tools":[{"type":"function","function":{"name":"echo","parameters":{"type":"object"}}}]})
    }
}

fn message(raw: &str, gemini: bool, stream: bool, tool: bool) -> Value {
    assert!(!raw.contains("[DONE]"), "{raw}");
    for absent in [
        "usage",
        "usageMetadata",
        "modelVersion",
        "tool_plan",
        "reasoning_content",
        "thoughtSignature",
        "finished thinking",
    ] {
        assert!(!raw.contains(absent), "{absent}: {raw}");
    }
    let values: Vec<Value> = if stream {
        raw.lines()
            .filter_map(|l| l.strip_prefix("data: "))
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    } else {
        vec![serde_json::from_str(raw).unwrap()]
    };
    if gemini {
        assert_eq!(values.len(), 1);
        assert_eq!(values[0]["candidates"][0]["finishReason"], "STOP");
        let msg = values[0]["candidates"][0]["content"].clone();
        assert_eq!(msg["role"], "model");
        return msg;
    }
    let expected = if tool { "TOOL_CALL" } else { "COMPLETE" };
    if !stream {
        assert_eq!(values[0]["finish_reason"], expected);
        return values[0]["message"].clone();
    }
    assert_eq!(values.first().unwrap()["type"], "message-start");
    assert_eq!(values.last().unwrap()["type"], "message-end");
    assert_eq!(values.last().unwrap()["delta"]["finish_reason"], expected);
    let mut msg = json!({"role":"assistant"});
    for event in values {
        match event["type"].as_str().unwrap() {
            "content-delta" => {
                msg["content"] =
                    json!([{"type":"text","text":event["delta"]["message"]["content"]["text"]}])
            }
            "tool-call-start" => {
                msg["tool_calls"] = json!([event["delta"]["message"]["tool_calls"]])
            }
            "tool-call-delta" => {
                msg["tool_calls"][0]["function"]["arguments"] =
                    event["delta"]["message"]["tool_calls"]["function"]["arguments"].clone()
            }
            _ => {}
        }
    }
    msg
}

fn append(body: &mut Value, gemini: bool, msg: Value, round: usize) {
    if gemini {
        let call = msg["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|p| p.get("functionCall"))
            .unwrap()
            .clone();
        let contents = body["contents"].as_array_mut().unwrap();
        contents.push(msg);
        contents.push(json!({"role":"user","parts":[{"functionResponse":{
            "id":call["id"],"name":call["name"],"response":{"result":round}}}]}));
    } else {
        let id = msg["tool_calls"][0]["id"].clone();
        let messages = body["messages"].as_array_mut().unwrap();
        messages.push(msg);
        let result = json!({"result":round}).to_string();
        messages.push(json!({"role":"tool","tool_call_id":id,
            "content":if round == 0 {json!(result)} else {json!([{"type":"text","text":result}])}}));
    }
}

#[test]
fn cursor_remote_native_json_sse_two_rounds_exact_scope_and_history() {
    run(|| async {
        let state = app_worker(true, worker(0)).await;
        for gemini in [true, false] {
            let scope = if gemini {
                "gemini.generateContent"
            } else {
                "cohere.chat"
            };
            let token = key(&state, scope, "cursor-remote").await;
            let wrong = key(&state, "chat.completions", "cursor-remote").await;
            let other = key(&state, scope, "cursor-remote").await;
            for stream in [false, true] {
                let path = endpoint(gemini, stream);
                let mut request = body(gemini, stream);
                assert_ne!(
                    post_to(&state, &wrong, path, request.clone()).await.0,
                    StatusCode::OK
                );
                for round in 0..2 {
                    let (status, raw) = post_to(&state, &token, path, request.clone()).await;
                    assert_eq!(status, StatusCode::OK, "{raw}");
                    let msg = message(&raw, gemini, stream, true);
                    assert!(raw.contains(&format!("prelude {round}")));
                    append(&mut request, gemini, msg, round);
                    let (status, raw) = post_to(&state, &other, path, request.clone()).await;
                    assert_ne!(status, StatusCode::OK, "{raw}");
                    let mut changed = request.clone();
                    if gemini {
                        changed["contents"][0]["parts"][0]["text"] = json!("changed");
                    } else {
                        changed["messages"][0]["content"] = json!("changed");
                    }
                    let (status, raw) = post_to(&state, &token, path, changed).await;
                    assert_ne!(status, StatusCode::OK, "{raw}");
                    assert!(raw.contains("continuation_mismatch"), "{raw}");
                }
                let (status, raw) = post_to(&state, &token, path, request.clone()).await;
                assert_eq!(status, StatusCode::OK, "{raw}");
                message(&raw, gemini, stream, false);
                assert!(raw.contains("native complete 中文🦀"));
                assert_ne!(
                    post_to(&state, &token, path, request).await.0,
                    StatusCode::OK
                );
            }
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_native_relay_cannot_claim_a_different_protocol_pause() {
    run(|| async {
        let state = app_worker(true, worker(0)).await;
        let token = key(&state, "relay", "cursor-remote").await;
        for gemini in [true, false] {
            let mut request = body(gemini, false);
            let (status, raw) =
                post_to(&state, &token, endpoint(gemini, false), request.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            let msg = message(&raw, gemini, false, true);
            let other_msg = if gemini {
                let call = &msg["parts"][1]["functionCall"];
                json!({"role":"assistant","content":[{"type":"text","text":msg["parts"][0]["text"]}],
                    "tool_calls":[{"id":call["id"],"type":"function","function":{"name":call["name"],"arguments":call["args"].to_string()}}]})
            } else {
                let call = &msg["tool_calls"][0];
                json!({"role":"model","parts":[{"text":msg["content"][0]["text"]},
                    {"functionCall":{"id":call["id"],"name":call["function"]["name"],
                        "args":serde_json::from_str::<Value>(call["function"]["arguments"].as_str().unwrap()).unwrap()}}]})
            };
            let mut cross = body(!gemini, false);
            append(&mut cross, !gemini, other_msg, 0);
            let (status, raw) = post_to(&state, &token, endpoint(!gemini, false), cross).await;
            assert_ne!(status, StatusCode::OK, "{raw}");
            assert!(raw.contains("continuation_mismatch"), "{raw}");
            append(&mut request, gemini, msg, 0);
            let (status, raw) =
                post_to(&state, &token, endpoint(gemini, false), request.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            append(&mut request, gemini, message(&raw, gemini, false, true), 1);
            assert_eq!(
                post_to(&state, &token, endpoint(gemini, false), request)
                    .await
                    .0,
                StatusCode::OK
            );
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[path = "http_native_lifecycle_tests.rs"]
mod lifecycle;
