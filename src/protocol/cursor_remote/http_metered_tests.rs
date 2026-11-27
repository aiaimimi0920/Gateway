//! Router/account/audit proof with synthetic usage, not real Cursor account acceptance.
use super::*;
use crate::protocol::{bedrock_converse, canonical::TokenUsage};

fn usage() -> TokenUsage {
    TokenUsage {
        prompt_tokens: 10,
        completion_tokens: 5,
        total_tokens: 15,
        cache_read_input_tokens: Some(2),
        cache_creation_input_tokens: Some(1),
    }
}

fn worker(known: bool, sends: Arc<std::sync::atomic::AtomicUsize>) -> Runtime {
    Runtime::fixture(move |input, hooks| {
        let sends = sends.clone();
        Box::pin(async move {
            assert!(input["prompt"].as_str().unwrap().contains("hello"));
            assert!(input.get("tools").is_none());
            hooks.authorize().await?;
            hooks.sent()?;
            sends.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(WorkerOutput {
                text: "metered 中文🦀".into(),
                thinking: None,
                requested_model: "default".into(),
                usage: known.then(usage),
            })
        })
    })
}

fn request(kind: usize, stream: bool) -> (String, Value, &'static str) {
    match kind {
        0 => (
            format!(
                "/model/cursor-personal/converse{}",
                if stream { "-stream" } else { "" }
            ),
            json!({"system":[{"text":"be concise"}],"messages":[{"role":"user","content":[{"text":"hello"}]}]}),
            "bedrock.converse",
        ),
        _ => (
            format!(
                "/api/v1/services/aigc/{}-generation/generation",
                if kind == 1 { "text" } else { "multimodal" }
            ),
            json!({"model":"cursor-personal","input":{"messages":[{"role":"user",
                "content":if kind == 1 {json!("hello")} else {json!([{"text":"hello"}])}}]},
                "parameters":{"result_format":"message"}}),
            if kind == 1 {
                "dashscope.text"
            } else {
                "dashscope.multimodal"
            },
        ),
    }
}

async fn send(
    state: &Arc<AppState>,
    key: &str,
    kind: usize,
    stream: bool,
    body: Value,
) -> (StatusCode, Vec<u8>) {
    let (path, _, _) = request(kind, stream);
    let mut builder = Request::post(path)
        .header("authorization", format!("Bearer {key}"))
        .header("content-type", "application/json");
    if kind != 0 && stream {
        builder = builder.header("x-dashscope-sse", "enable");
    }
    let response = Box::pin(
        crate::http::router::build_router(state.clone())
            .oneshot(builder.body(Body::from(body.to_string())).unwrap()),
    )
    .await
    .unwrap();
    let status = response.status();
    if status == StatusCode::OK {
        assert_eq!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap(),
            if !stream {
                "application/json"
            } else if kind == 0 {
                "application/vnd.amazon.eventstream"
            } else {
                "text/event-stream"
            }
        );
    }
    (
        status,
        to_bytes(response.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
}

#[test]
fn cursor_remote_metered_json_and_native_stream_preserve_usage_and_scope() {
    run(|| async {
        let sends = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let state = app_worker(true, worker(true, sends.clone())).await;
        for kind in 0..3 {
            let (_, _, scope) = request(kind, false);
            let token = key(&state, scope, "cursor-remote").await;
            let wrong = key(&state, "chat.completions", "cursor-remote").await;
            for stream in [false, true] {
                let (_, body, _) = request(kind, stream);
                assert_eq!(
                    send(&state, &wrong, kind, stream, body.clone()).await.0,
                    StatusCode::FORBIDDEN
                );
                let (status, bytes) = send(&state, &token, kind, stream, body).await;
                assert_eq!(
                    status,
                    StatusCode::OK,
                    "{}",
                    String::from_utf8_lossy(&bytes)
                );
                if kind == 0 && stream {
                    let decoded = bedrock_converse::decode_eventstream(bytes, "default").unwrap();
                    assert_eq!(decoded.usage, Some(usage()));
                    assert_eq!(decoded.text, "metered 中文🦀");
                } else {
                    let raw = String::from_utf8(bytes).unwrap();
                    let json_text = if stream {
                        raw.lines().find_map(|s| s.strip_prefix("data: ")).unwrap()
                    } else {
                        raw.as_str()
                    };
                    let value: Value = serde_json::from_str(json_text).unwrap();
                    if kind == 0 {
                        assert_eq!(value["usage"]["totalTokens"], 15);
                        assert!(value["metrics"]["latencyMs"].is_u64());
                    } else {
                        assert_eq!(value["usage"]["total_tokens"], 15);
                        assert_eq!(value["output"]["choices"][0]["finish_reason"], "stop");
                    }
                    assert!(raw.contains("metered 中文🦀"));
                    assert!(!raw.contains("modelVersion"));
                }
            }
        }
        assert_eq!(sends.load(std::sync::atomic::Ordering::SeqCst), 6);
        let rows = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let rows = state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                // Scope authentication fails before request-audit admission.
                // Only the six authenticated inference requests have stage audits.
                if rows.len() == 6 && rows.iter().all(|r| r.completed_at.is_some()) {
                    break rows;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        for row in rows {
            assert_eq!(row.upstream_status, Some(200));
            assert_eq!(row.total_tokens, Some(15));
            assert_eq!(
                row.route_trace.as_ref().unwrap()["realCredentialRef"],
                "cursor-account"
            );
            assert_eq!(row.route_trace.as_ref().unwrap()["routeAttemptCount"], 1);
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_metered_missing_usage_fails_instead_of_fabricating_success() {
    run(|| async {
        let state = app_worker(
            true,
            worker(false, Arc::new(std::sync::atomic::AtomicUsize::new(0))),
        )
        .await;
        let token = key(&state, "relay", "cursor-remote").await;
        for kind in 0..3 {
            for stream in [false, true] {
                let (_, body, _) = request(kind, stream);
                let (status, bytes) = send(&state, &token, kind, stream, body).await;
                assert_ne!(status, StatusCode::OK);
                assert!(String::from_utf8(bytes)
                    .unwrap()
                    .contains("cursor_remote_token_usage_unavailable"));
            }
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_metered_controls_tools_and_remote_images_never_send() {
    run(|| async {
        let sends = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let state = app_worker(true, worker(true, sends.clone())).await;
        let token = key(&state, "relay", "cursor-remote").await;
        for kind in 0..3 {
            let (_, base, _) = request(kind, false);
            let changes = if kind == 0 {
                vec![
                    json!({"inferenceConfig":{"maxTokens":16}}),
                    json!({"toolConfig":{"tools":[]}}),
                    json!({"additionalModelRequestFields":{"temperature":0}}),
                ]
            } else {
                vec![
                    json!({"parameters":{"max_tokens":16}}),
                    json!({"parameters":{"tools":[]}}),
                    json!({"parameters":{"temperature":0}}),
                ]
            };
            for change in changes {
                let mut body = base.clone();
                body.as_object_mut()
                    .unwrap()
                    .extend(change.as_object().unwrap().clone());
                assert_ne!(
                    send(&state, &token, kind, false, body).await.0,
                    StatusCode::OK
                );
            }
        }
        let (_, mut body, _) = request(2, false);
        body["input"]["messages"][0]["content"] =
            json!([{"text":"hello"},{"image":"https://example.test/a.png"}]);
        assert_ne!(send(&state, &token, 2, false, body).await.0, StatusCode::OK);
        assert_eq!(sends.load(std::sync::atomic::Ordering::SeqCst), 0);
        for header in [
            "x-amzn-bedrock-guardrailidentifier",
            "x-amzn-bedrock-guardrailversion",
        ] {
            let (path, body, _) = request(0, false);
            let request = Request::post(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header(header, "must-not-ignore")
                .body(Body::from(body.to_string()))
                .unwrap();
            let response =
                Box::pin(crate::http::router::build_router(state.clone()).oneshot(request))
                    .await
                    .unwrap();
            assert_ne!(response.status(), StatusCode::OK);
            let raw = to_bytes(response.into_body(), 65536).await.unwrap();
            assert!(String::from_utf8_lossy(&raw).contains("cursor_remote_unsupported_request"));
        }
        assert_eq!(sends.load(std::sync::atomic::Ordering::SeqCst), 0);
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_metered_png_jpeg_reach_native_image_decoder() {
    run(|| async {
        let state = app_worker(
            true,
            Runtime::fixture(|input, hooks| {
                Box::pin(async move {
                    let images =
                        super::super::upstream::validate_images(input.get("images")).unwrap();
                    assert_eq!(images.len(), 1);
                    assert!(!input["prompt"].as_str().unwrap().contains("base64"));
                    hooks.authorize().await?;
                    hooks.sent()?;
                    Ok(WorkerOutput {
                        text: "image accepted".into(),
                        thinking: None,
                        requested_model: "default".into(),
                        usage: Some(usage()),
                    })
                })
            }),
        )
        .await;
        let token = key(&state, "relay", "cursor-remote").await;
        for kind in [0, 2] {
            for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
                let url = super::super::inline_images::tests::url(format, 20);
                let (header, data) = url.split_once(',').unwrap();
                let format = if header.contains("png") {
                    "png"
                } else {
                    "jpeg"
                };
                for stream in [false, true] {
                    let (_, mut body, _) = request(kind, stream);
                    if kind == 0 {
                        body["messages"][0]["content"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"image":{"format":format,"source":{"bytes":data}}}));
                    } else {
                        body["input"]["messages"][0]["content"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"image":url}));
                    }
                    let (status, bytes) = send(&state, &token, kind, stream, body).await;
                    assert_eq!(
                        status,
                        StatusCode::OK,
                        "{}",
                        String::from_utf8_lossy(&bytes)
                    );
                }
            }
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
