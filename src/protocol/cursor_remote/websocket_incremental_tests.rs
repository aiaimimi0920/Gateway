//! A live synthetic pure-Rust worker across delta/full/delta turns, including inline media.
use super::*;
use crate::protocol::cursor_remote::{
    inline_images::tests::{message, url},
    upstream::validate_images,
};

fn multi_worker() -> Runtime {
    Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            if input.get("tools").is_none() {
                return Ok(WorkerOutput {
                    usage: None,
                    text: "new plain turn".into(),
                    thinking: None,
                    requested_model: "default".into(),
                });
            }
            let images = validate_images(input.get("images"))
                .map_err(|e| crate::protocol::cursor_remote::error(e.code))?;
            if images.len() != 1
                || input["tools"].as_array().map(Vec::len) != Some(2)
                || !input["prompt"]
                    .as_str()
                    .is_some_and(|p| p.contains("frozen policy"))
            {
                return Err(crate::protocol::cursor_remote::error("worker_failed"));
            }
            let patch = json!({"input":" \r\n补丁\n "}).to_string();
            let final_edit = json!({"input":"final edit"}).to_string();
            for (call_id, name, args) in [
                ("call_a", "echo", "{}"),
                ("call_b", "edit", patch.as_str()),
                ("call_c", "edit", final_edit.as_str()),
            ] {
                let output = hooks
                    .tool(WorkerToolCall {
                        call_id: call_id.into(),
                        name: name.into(),
                        arguments: args.into(),
                        text: format!("prelude {call_id}"),
                        thinking: Some(format!("public {call_id}")),
                        requested_model: "default".into(),
                    })
                    .await?;
                if output.call_id != call_id || output.output != "ok" {
                    return Err(crate::protocol::cursor_remote::error("worker_failed"));
                }
                hooks.continued()?;
            }
            Ok(WorkerOutput {
                usage: None,
                text: "incremental done".into(),
                thinking: Some("public final".into()),
                requested_model: "default".into(),
            })
        })
    })
}

fn full(history: &[Value]) -> Value {
    json!({"type":"response.create","model":"cursor-personal","store":false,"input":history,
        "instructions":"frozen policy","tool_choice":"auto","parallel_tool_calls":false,
        "tools":[{"type":"function","name":"echo","parameters":{"type":"object"}},
            {"type":"custom","name":"edit","format":{"type":"text"}}]})
}
fn result(event: &Value) -> Value {
    let call = event["response"]["output"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            matches!(
                item["type"].as_str(),
                Some("function_call" | "custom_tool_call")
            )
        })
        .unwrap();
    json!({"type":if call["type"] == "custom_tool_call" {"custom_tool_call_output"} else {"function_call_output"},
        "call_id":call["call_id"],"output":"ok"})
}
fn delta(event: &Value) -> Value {
    json!({"type":"response.create","model":"cursor-personal","store":false,
        "previous_response_id":event["response"]["id"],"input":[result(event)]})
}
fn append(history: &mut Vec<Value>, event: &Value) {
    assert_eq!(event["type"], "response.completed", "{event}");
    assert!(event["response"]["id"]
        .as_str()
        .unwrap()
        .starts_with("resp_"));
    let call_id = result(event)["call_id"].as_str().unwrap().to_owned();
    assert_eq!(event["response"]["output"][0]["type"], "reasoning");
    assert_eq!(
        event["response"]["output"][0]["content"][0]["text"],
        format!("public {call_id}")
    );
    history.extend(event["response"]["output"].as_array().unwrap().clone());
    history.push(result(event));
}

#[test]
fn cursor_remote_websocket_incremental_full_history_and_images_share_one_run() {
    run(|| async {
        let state = app_worker(true, multi_worker()).await;
        let token = key(&state, "responses", "cursor-remote").await;
        let other_token = key(&state, "responses", "cursor-remote").await;
        let server = Server::start(state.clone()).await;
        let mut socket = server.connect(&token).await;
        let mut other = server.connect(&token).await;
        let mut other_key = server.connect(&other_token).await;
        let mut history = vec![message(&url(image::ImageFormat::Png, 10), false)];
        let first = turn(&mut socket, full(&history)).await;
        append(&mut history, &first);
        let mut first_delta = delta(&first);
        first_delta["input"][0]["output"] = json!([
            {"type":"input_text","text":"o"},{"type":"input_text","text":"k"}
        ]);
        let second = turn(&mut socket, first_delta).await;
        append(&mut history, &second);
        assert_ne!(first["response"]["id"], second["response"]["id"]);
        assert_eq!(result(&second)["type"], "custom_tool_call_output");
        assert!(second["response"]["output"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["input"] == " \r\n补丁\n "));
        let good = delta(&second);
        let stale = turn(&mut socket, delta(&first)).await;
        assert_eq!(stale["type"], "error");
        assert!(stale.to_string().contains("current delivered tool pause"));
        let mut wrong_call = good.clone();
        wrong_call["input"][0]["call_id"] = json!("call_wrong");
        let rejected_call = turn(&mut socket, wrong_call).await;
        assert_eq!(rejected_call["type"], "error");
        assert!(rejected_call
            .to_string()
            .contains("Cursor remote execution did not complete"));
        let mut wrong_kind = good.clone();
        wrong_kind["input"][0]["type"] = json!("function_call_output");
        let rejected_kind = turn(&mut socket, wrong_kind).await;
        assert_eq!(rejected_kind["type"], "error");
        assert!(rejected_kind
            .to_string()
            .contains("Cursor remote execution did not complete"));
        let mut bad = vec![];
        for id in [json!("resp_missing"), json!(null), json!(123)] {
            let mut request = good.clone();
            request["previous_response_id"] = id;
            bad.push(request);
        }
        let mut no_id = good.clone();
        no_id
            .as_object_mut()
            .unwrap()
            .remove("previous_response_id");
        bad.push(no_id);
        for (field, value) in [
            ("tools", full(&history)["tools"].clone()),
            ("instructions", json!("frozen policy")),
            ("tool_choice", json!("none")),
            ("parallel_tool_calls", json!(false)),
            ("store", json!(true)),
            ("stream_id", json!("main")),
        ] {
            let mut request = good.clone();
            request[field] = value;
            bad.push(request);
        }
        for (field, value) in [
            ("output", json!("x".repeat(65537))),
            ("output", json!("\0".repeat(30000))),
            ("unknown", json!(true)),
        ] {
            let mut request = good.clone();
            request["input"][0][field] = value;
            bad.push(request);
        }
        let mut extra = good.clone();
        extra["input"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"new task"}));
        bad.push(extra);
        for request in bad {
            assert_eq!(turn(&mut socket, request).await["type"], "error");
        }
        for other_socket in [&mut other, &mut other_key] {
            let rejected = turn(other_socket, good.clone()).await;
            assert_eq!(rejected["type"], "error");
            assert!(rejected
                .to_string()
                .contains("current delivered tool pause"));
        }
        let mut http = good;
        http.as_object_mut().unwrap().remove("type");
        let (http_status, http_error) = post(&state, &token, http).await;
        assert_ne!(http_status, StatusCode::OK);
        assert!(
            http_error.contains("cursor_remote_previous_response_unavailable"),
            "{http_error}"
        );
        let mut tampered = history.clone();
        tampered[0] = message(&url(image::ImageFormat::Png, 20), false);
        assert_eq!(turn(&mut socket, full(&tampered)).await["type"], "error");
        let third = turn(&mut socket, full(&history)).await;
        append(&mut history, &third);
        assert_ne!(second["response"]["id"], third["response"]["id"]);
        assert_eq!(result(&third)["call_id"], "call_c");
        assert_eq!(result(&third)["type"], "custom_tool_call_output");
        assert_eq!(turn(&mut socket, delta(&second)).await["type"], "error");
        let final_request = delta(&third);
        let done = turn(&mut socket, final_request.clone()).await;
        assert_eq!(done["type"], "response.completed", "{done}");
        assert!(done.to_string().contains("incremental done"));
        assert_eq!(
            done["response"]["output"][0]["content"][0]["text"],
            "public final"
        );
        assert!(!done.to_string().contains("public call_"));
        assert!(done["response"]["id"]
            .as_str()
            .unwrap()
            .starts_with("resp_"));
        assert_ne!(done["response"]["id"], third["response"]["id"]);
        assert!(done["response"]["output"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| !matches!(
                item["type"].as_str(),
                Some("function_call" | "custom_tool_call")
            )));
        assert_eq!(
            turn(&mut socket, final_request.clone()).await["type"],
            "error"
        );
        let mut after_done = final_request;
        after_done["previous_response_id"] = done["response"]["id"].clone();
        assert_eq!(turn(&mut socket, after_done).await["type"], "error");
        let plain = turn(
            &mut socket,
            json!({"type":"response.create","model":"cursor-personal",
            "store":false,"input":"new task"}),
        )
        .await;
        assert_eq!(plain["type"], "response.completed", "{plain}");
        assert!(plain.to_string().contains("new plain turn"));
        for mut connection in [socket, other, other_key] {
            connection.close(None).await.unwrap();
        }
        state.lifecycle.cursor_remote.join().await;
        drop(server);
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
