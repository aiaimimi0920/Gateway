use super::*;

#[test]
fn cursor_remote_native_unconsumed_body_cancels_its_run() {
    run(|| async {
        let state = app_worker(true, worker(0)).await;
        let token = key(&state, "relay", "cursor-remote").await;
        for gemini in [true, false] {
            for stream in [false, true] {
                let request = Request::post(endpoint(gemini, stream))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body(gemini, stream).to_string()))
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
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_native_text_and_none_do_not_install_tools_or_thinking_fields() {
    run(|| async {
        let state = app_worker(
            true,
            Runtime::fixture(|input, hooks| {
                Box::pin(async move {
                    assert!(input.get("tools").is_none());
                    assert!(input["prompt"].as_str().unwrap().contains("hello"));
                    hooks.authorize().await?;
                    hooks.sent()?;
                    Ok(WorkerOutput {
                        usage: None,
                        text: "native text".into(),
                        thinking: Some("private to unmapped wire".into()),
                        requested_model: "default".into(),
                    })
                })
            }),
        )
        .await;
        let token = key(&state, "relay", "cursor-remote").await;
        for gemini in [true, false] {
            for stream in [false, true] {
                for none in [true, false] {
                    let mut request = body(gemini, stream);
                    if none {
                        if gemini {
                            request["toolConfig"] =
                                json!({"functionCallingConfig":{"mode":"NONE"}});
                        } else {
                            request["tool_choice"] = json!("NONE");
                        }
                    } else {
                        request.as_object_mut().unwrap().remove("tools");
                    }
                    let (status, raw) =
                        post_to(&state, &token, endpoint(gemini, stream), request).await;
                    assert_eq!(status, StatusCode::OK, "{raw}");
                    message(&raw, gemini, stream, false);
                    assert!(raw.contains("native text"));
                    assert!(!raw.contains("private to unmapped wire"));
                }
            }
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_gemini_images_are_bound_across_tool_turns() {
    run(|| async {
        use crate::protocol::cursor_remote::inline_images::tests::url;
        let state = app_worker(true, worker(1)).await;
        let token = key(&state, "gemini.generateContent", "cursor-remote").await;
        for (format, mime) in [
            (image::ImageFormat::Png, "image/png"),
            (image::ImageFormat::Jpeg, "image/jpeg"),
        ] {
            let image = url(format, 15).split_once(',').unwrap().1.to_owned();
            let mut request = body(true, false);
            request["contents"][0]["parts"]
                .as_array_mut()
                .unwrap()
                .push(json!({"inlineData":{"mimeType":mime,"data":image}}));
            let (status, raw) =
                post_to(&state, &token, endpoint(true, false), request.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            append(&mut request, true, message(&raw, true, false, true), 0);
            let mut tampered = request.clone();
            tampered["contents"][0]["parts"][1]["inlineData"]["data"] =
                json!(url(format, 20).split_once(',').unwrap().1);
            assert_ne!(
                post_to(&state, &token, endpoint(true, false), tampered)
                    .await
                    .0,
                StatusCode::OK
            );
            let (status, raw) =
                post_to(&state, &token, endpoint(true, false), request.clone()).await;
            assert_eq!(status, StatusCode::OK, "{raw}");
            append(&mut request, true, message(&raw, true, false, true), 1);
            assert_eq!(
                post_to(&state, &token, endpoint(true, false), request)
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
fn cursor_remote_native_unsupported_controls_have_zero_dispatches() {
    run(|| async {
        let state = app_worker(
            true,
            Runtime::fixture(|_, _| panic!("rejected wire reached native worker")),
        )
        .await;
        let token = key(&state, "relay", "cursor-remote").await;
        let mut cases = Vec::new();
        for (gemini, field, value) in [
            (true, "generationConfig", json!({"maxOutputTokens":16})),
            (true, "generationConfig", json!({"temperature":0.1})),
            (
                true,
                "generationConfig",
                json!({"responseMimeType":"application/json"}),
            ),
            (
                true,
                "toolConfig",
                json!({"functionCallingConfig":{"mode":"ANY"}}),
            ),
            (
                true,
                "toolConfig",
                json!({"functionCallingConfig":{"mode":"VALIDATED"}}),
            ),
            (true, "safetySettings", json!([])),
            (
                true,
                "system_instruction",
                json!({"parts":[{"text":"dropped"}]}),
            ),
            (false, "max_tokens", json!(16)),
            (false, "temperature", json!(0.1)),
            (false, "thinking", json!({"type":"enabled"})),
            (false, "tool_choice", json!("REQUIRED")),
            (false, "tool_choice", json!("AUTO")),
            (false, "response_format", json!({"type":"json_object"})),
        ] {
            let mut request = body(gemini, false);
            request[field] = value;
            cases.push((endpoint(gemini, false), request));
        }
        cases.push((
            "/v1beta/models/cursor-personal:generateContent?alt=sse",
            body(true, false),
        ));
        cases.push(("/v2/chat?unsupported=true", body(false, false)));
        for (path, request) in &cases {
            let (status, raw) = post_to(&state, &token, path, request.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {raw}");
            assert!(raw.contains("cursor_remote_unsupported"), "{raw}");
        }
        let rows = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let rows = state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == cases.len() && rows.iter().all(|r| r.completed_at.is_some()) {
                    break rows;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(rows.iter().all(|r| r.provider_account_id.is_none()
            && r.route_trace.as_ref().unwrap()["routeAttemptCount"] == 0));
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
