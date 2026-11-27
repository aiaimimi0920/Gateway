//! End-to-end router/pure-Rust worker fixtures, not evidence of upstream image acceptance.
use super::*;
use crate::protocol::cursor_remote::{
    inline_images::tests::{body, message, url},
    upstream::validate_images,
};

fn text_image_worker() -> Runtime {
    Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            let images =
                validate_images(input.get("images")).map_err(|e| super::super::error(e.code))?;
            if images.len() != 1
                || input.get("tools").is_some()
                || input["prompt"]
                    .as_str()
                    .is_some_and(|p| p.contains("base64"))
            {
                return Err(super::super::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            Ok(WorkerOutput {
                usage: None,
                text: "image received".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

fn tools_image_worker() -> Runtime {
    Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            let images =
                validate_images(input.get("images")).map_err(|e| super::super::error(e.code))?;
            if images.len() != 2 || input["tools"].as_array().map(Vec::len) != Some(1) {
                return Err(super::super::error("worker_failed"));
            }
            hooks.authorize().await?;
            hooks.sent()?;
            let output = hooks
                .tool(WorkerToolCall {
                    call_id: "call_image".into(),
                    name: "echo".into(),
                    arguments: "{}".into(),
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if output.output != "ok" {
                return Err(super::super::error("worker_failed"));
            }
            hooks.continued()?;
            Ok(WorkerOutput {
                usage: None,
                text: "image tool completed".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

#[test]
fn cursor_remote_http_inline_images_text_and_none_on_both_protocols() {
    run(|| async {
        let state = app_worker(true, text_image_worker()).await;
        let token = key(&state, "relay", "cursor-remote").await;
        for chat in [false, true] {
            let endpoint = if chat {
                "/v1/chat/completions"
            } else {
                "/v1/responses"
            };
            for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
                for (stream, choice) in [(false, None), (true, Some("none"))] {
                    let image = message(&url(format, 15), chat);
                    let (status, raw) = post_to(
                        &state,
                        &token,
                        endpoint,
                        body(chat, json!([image]), choice, stream),
                    )
                    .await;
                    assert_eq!(status, StatusCode::OK, "{raw}");
                    assert!(raw.contains("image received"), "{raw}");
                }
            }
            let invalid = "data:image/png;base64,iVBORw0KGgo=";
            let (status, _) = post_to(
                &state,
                &token,
                endpoint,
                body(chat, json!([message(invalid, chat)]), None, false),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_http_image_tampering_does_not_consume_same_run_continuation() {
    run(|| async {
        let state = app_worker(true, tools_image_worker()).await;
        let token = key(&state, "relay", "cursor-remote").await;
        for chat in [false, true] {
            let endpoint = if chat {
                "/v1/chat/completions"
            } else {
                "/v1/responses"
            };
            let mut image = message(&url(image::ImageFormat::Png, 0), chat);
            let second = message(&url(image::ImageFormat::Jpeg, 255), chat)["content"][1].clone();
            image["content"].as_array_mut().unwrap().push(second);
            let mut history = vec![image];
            let (status, raw) = post_to(
                &state,
                &token,
                endpoint,
                body(chat, json!(history), Some("auto"), false),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            if chat {
                let response: Value = serde_json::from_str(&raw).unwrap();
                history.push(response["choices"][0]["message"].clone());
                history.push(json!({"role":"tool","tool_call_id":"call_image","content":"ok"}));
            } else {
                history.push(json!({"type":"function_call","call_id":"call_image","name":"echo","arguments":"{}"}));
                history.push(
                    json!({"type":"function_call_output","call_id":"call_image","output":"ok"}),
                );
            }
            for kind in ["delete", "swap", "replace", "move", "text"] {
                let mut tampered = history.clone();
                let parts = tampered[0]["content"].as_array_mut().unwrap();
                match kind {
                    "delete" => {
                        parts.remove(1);
                    }
                    "swap" => parts.swap(1, 2),
                    "replace" => {
                        parts[1] =
                            message(&url(image::ImageFormat::Png, 30), chat)["content"][1].clone();
                    }
                    "move" => parts.swap(0, 1),
                    _ => {
                        parts[1] = json!({"type":if chat {"text"} else {"input_text"},
                        "text":"image/png sha256: pretend image"});
                    }
                }
                let (status, raw) = post_to(
                    &state,
                    &token,
                    endpoint,
                    body(chat, json!(tampered), Some("auto"), false),
                )
                .await;
                assert_ne!(status, StatusCode::OK, "{kind}: {raw}");
                assert!(
                    raw.contains("cursor_remote_continuation_mismatch"),
                    "{kind}: {raw}"
                );
            }
            let mut oversize_result = history.clone();
            let last = oversize_result.last_mut().unwrap();
            last[if chat { "content" } else { "output" }] = json!("\0".repeat(22000));
            assert_eq!(
                post_to(
                    &state,
                    &token,
                    endpoint,
                    body(chat, json!(oversize_result), Some("auto"), false)
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
            let request = body(chat, json!(history), Some("auto"), false);
            let (status, raw) = post_to(&state, &token, endpoint, request.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            assert!(raw.contains("image tool completed"));
            assert_ne!(
                post_to(&state, &token, endpoint, request).await.0,
                StatusCode::OK
            );
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
