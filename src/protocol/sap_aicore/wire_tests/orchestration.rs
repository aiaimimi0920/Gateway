//! Orchestration v2 sends one fixed-model request and only consumes the authoritative final result.
use super::*;

pub(super) async fn fixture(peer: &peer::Peer, fallback: bool) -> Fixture {
    Fixture::configured(
        peer,
        fallback,
        super::super::test_support::BASE,
        KEY,
        json!({"sap_orchestration_model":"gpt-4.1-mini"}),
    )
    .await
}

fn reply() -> Value {
    json!({"request_id":"orch-request-id","intermediate_results":{"llm":{"choices":[{"message":{"content":"not-public"}}], "usage":{"total_tokens":99}}},
        "final_result":{"id":"orch-final-id","object":"chat.completion","created":123,"model":"tenant-internal-model",
        "choices":[{"index":0,"message":{"role":"assistant","content":"合成答案"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}})
}

#[tokio::test]
async fn sap_aicore_orchestration_chat_final_result_and_fixed_model() {
    Box::pin(async {
        let peer = peer::Peer::raw(reply().to_string().into_bytes()).await;
        let f = fixture(&peer, false).await;
        let request = json!({"model":MODEL,"max_tokens":32,"stream":false,"user":"local-only",
            "messages":[{"role":"system","content":"规则"},{"role":"user","content":"literal {{?placeholder}}"},
                {"role":"assistant","content":"已有回答"},{"role":"user","content":"再问一次"}]});
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = f.open("chat/completions", request.clone()).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            let public: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(public["model"], MODEL);
            assert_eq!(public["choices"][0]["message"]["content"], "合成答案");
            assert!(!text.contains("not-public") && !text.contains("tenant-internal-model"));
            let audits = f.audits(1).await;
            assert_identity(&audits[0]);
            assert_eq!(audits[0].status, "completed");
            assert_eq!(audits[0].total_tokens, Some(5));
            assert_eq!(f.remaining().await, 1);
        }).await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        {
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            let (path, headers, body) = &calls[0];
            assert_eq!(path, "/v2/inference/deployments/deployment1/v2/completion");
            assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
            assert_eq!(headers["AI-Resource-Group"], "group-a");
            assert!(!headers.contains_key("cookie"));
            assert_eq!(*body, json!({"config":{"modules":{"prompt_templating":{
                "model":{"name":"gpt-4.1-mini","params":{"max_completion_tokens":32}}}}},
                "messages_history":request["messages"]}));
        }
        f.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_orchestration_refuses_unsupported_controls_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::raw(reply().to_string().into_bytes()).await;
        let f = fixture(&peer, false).await;
        let mut cases = vec![
            ("responses", body("responses", false)),
            ("messages", body("messages", false)),
            ("responses", body("responses", true)),
        ];
        for (key, value) in [
            ("store", json!(true)),
            ("n", json!(2)),
            ("max_tokens", json!(0)),
            ("config", json!({"model":"different"})),
            ("tools", json!([])),
            ("reasoning_effort", json!("high")),
        ] {
            let mut request = body("chat/completions", false);
            request[key] = value;
            cases.push(("chat/completions", request));
        }
        for options in [
            json!({"include_usage":false}),
            json!({"include_usage":"true"}),
            json!({"include_usage":true,"vendor":7}),
        ] {
            let mut request = body("chat/completions", true);
            request["stream_options"] = options;
            cases.push(("chat/completions", request));
        }
        let mut tools = body("chat/completions", true);
        tools["tools"] = json!([]);
        cases.push(("chat/completions", tools));
        super::super::test_support::origin(peer.origin.clone(), async {
            for (endpoint, request) in cases {
                let response = f.open(endpoint, request).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{endpoint}: {text}");
                assert!(text.contains("invalid_sap_aicore"), "{text}");
            }
            assert_eq!(f.remaining().await, 2);
        })
        .await;
        assert!(peer.oauth.lock().unwrap().is_empty());
        assert!(peer.calls.lock().unwrap().is_empty());
        f.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_orchestration_missing_final_result_never_uses_intermediate() {
    Box::pin(async {
        let final_only = reply()["final_result"].clone();
        let peer = peer::Peer::raw(
            json!({"request_id":"orch-failed","intermediate_results":{"llm":final_only}})
                .to_string()
                .into_bytes(),
        )
        .await;
        let f = fixture(&peer, false).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let response = f
                .open("chat/completions", body("chat/completions", false))
                .await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::BAD_GATEWAY, "{text}");
            assert!(!text.contains("合成答案"));
            assert_ne!(f.audits(1).await[0].status, "completed");
            assert_eq!(f.remaining().await, 2);
        })
        .await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        assert_eq!(peer.calls.lock().unwrap().len(), 1);
        f.finish().await;
        peer.stop().await;
    })
    .await;
}

fn stream_frame(content: &str, finish: &str, usage: Value) -> String {
    let mut result = json!({"id":"orch-final-id","object":"chat.completion.chunk","created":123,
        "model":"tenant-internal-model","choices":[{"index":0,
            "delta":{"role":"assistant","content":content},"finish_reason":finish}]});
    if !usage.is_null() {
        result["usage"] = usage;
    }
    let frame = json!({"request_id":"orch-request-id",
        "intermediate_results":{"llm":{"choices":[{"delta":{"content":"synthetic-private-stage"}}],
            "usage":{"prompt_tokens":100,"completion_tokens":100,"total_tokens":200}}},
        "final_result":result});
    format!("data: {frame}\r\n\r\n")
}

#[tokio::test]
async fn sap_aicore_orchestration_stream_authoritative_final_usage() {
    Box::pin(async {
        let initial = json!({"request_id":"orch-request-id","intermediate_results":{"templating":[{"content":"synthetic-private-stage"}]},
            "final_result":{"id":"","object":"","created":0,"model":"","choices":[{"index":0,"delta":{"role":"","content":""},"finish_reason":""}]}});
        let first = stream_frame("合成答", "", Value::Null);
        let last = stream_frame("案", "stop", json!({"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}));
        let peer = peer::Peer::raw(format!("data: {initial}\n\n{first}{last}data: [DONE]\n\n").into_bytes()).await;
        let f = fixture(&peer, false).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut request = body("chat/completions", true);
            request["stream_options"] = json!({"include_usage":true});
            let response = f.open("chat/completions", request).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            assert!(text.contains("合成答") && text.contains("案"), "{text}");
            assert!(!text.contains("synthetic-private-stage") && !text.contains("tenant-internal-model"));
            let frames: Vec<Value> = text.lines().filter_map(|line| line.strip_prefix("data: "))
                .filter(|data| *data != "[DONE]").map(|data| serde_json::from_str(data).unwrap()).collect();
            assert!(frames.iter().all(|frame| frame["model"] == MODEL), "{text}");
            assert_eq!(frames.iter().filter(|frame| frame["usage"]["total_tokens"] == 5).count(), 1, "{text}");
            assert_eq!(text.matches("[DONE]").count(), 1, "{text}");
            let audits = f.audits(1).await;
            assert_identity(&audits[0]);
            assert_eq!(audits[0].status, "completed");
            assert_eq!(audits[0].total_tokens, Some(5));
            assert_eq!(f.remaining().await, 1);
        }).await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        {
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            let (path, headers, body) = &calls[0];
            assert_eq!(path, "/v2/inference/deployments/deployment1/v2/completion");
            assert_eq!(headers["accept"], "text/event-stream");
            assert_eq!(body["config"]["stream"], json!({"enabled":true}));
            let model = &body["config"]["modules"]["prompt_templating"]["model"];
            assert_eq!(*model, json!({"name":"gpt-4.1-mini","params":{"stream_options":{"include_usage":true}}}));
            assert!(body.get("stream").is_none() && body.get("stream_options").is_none());
        }
        f.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_orchestration_stream_invalid_tail_never_settles_or_replays() {
    Box::pin(async {
        let first = stream_frame("实时回答", "", Value::Null);
        let last = stream_frame("", "stop", json!({"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}));
        let intermediate_only = json!({"request_id":"orch-request-id","intermediate_results":{"llm":reply()["final_result"]}});
        for (wire, expects_delta) in [
            (format!("data: {intermediate_only}\n\ndata: [DONE]\n\n"), false),
            (format!("{first}data: [DONE]\n\n"), true),
            (format!("{first}{last}"), true),
            (format!("{first}{last}data: [DONE]\n"), true),
            (format!("{first}{last}data: [DONE]\n\ndata: {intermediate_only}\n\n"), true),
            (format!("{first}{last}{last}data: [DONE]\n\n"), true),
            (format!("{first}{}data: [DONE]\n\n",last.replace("\"total_tokens\":5", "\"total_tokens\":91")), true),
            (format!("{first}{last}data: {{\"error\":{{\"message\":\"synthetic-private-error\"}}}}\n\n"), true),
        ] {
            let peer = peer::Peer::raw(wire.into_bytes()).await;
            let f = fixture(&peer, true).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let res = f.open("chat/completions", body("chat/completions", true)).await;
                let mut stream = res.into_body();
                let mut text = Vec::new();
                tokio::time::timeout(Duration::from_secs(5), async {
                    while let Some(Ok(frame)) = stream.frame().await {
                        if let Ok(data) = frame.into_data() {
                            text.extend_from_slice(&data);
                        }
                    }
                }).await.unwrap();
                drop(stream);
                let text = String::from_utf8(text).unwrap();
                assert_eq!(text.contains("实时回答"), expects_delta, "{text}");
                for forbidden in ["[DONE]", "total_tokens", "\"finish_reason\":\"stop\"", "synthetic-private", "合成答案"] {
                    assert!(!text.contains(forbidden), "{text}");
                }
                assert_ne!(f.audits(1).await[0].status, "completed");
                assert_eq!(f.remaining().await, 2);
                assert_eq!(peer.oauth.lock().unwrap().len(), 1);
                assert_eq!(peer.calls.lock().unwrap().len(), 1);
                peer.wait_closed().await;
            }).await;
            f.finish().await;
            peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn sap_aicore_orchestration_stream_cancel_closes_source_and_refunds() {
    Box::pin(async {
        let bytes = stream_frame("实时回答", "", Value::Null).into_bytes();
        let peer = peer::Peer::raw_held(bytes).await;
        let f = fixture(&peer, true).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let res = f
                .open("chat/completions", body("chat/completions", true))
                .await;
            assert_eq!(res.status(), StatusCode::OK);
            let mut stream = res.into_body();
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let data = stream.frame().await.unwrap().unwrap().into_data().unwrap();
                    if String::from_utf8_lossy(&data).contains("实时回答") {
                        break;
                    }
                }
            })
            .await
            .unwrap();
            drop(stream);
            assert_eq!(f.audits(1).await[0].status, "cancelled");
            assert_eq!(f.remaining().await, 2);
            assert_eq!(peer.calls.lock().unwrap().len(), 1);
            peer.wait_closed().await;
        })
        .await;
        f.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_orchestration_inline_images_public_json_and_sse() {
    Box::pin(async {
        let stream = stream_frame("answer", "stop", json!({"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}));
        let peer = peer::Peer::raw_sequence(vec![reply().to_string().into_bytes(),
            format!("{stream}data: [DONE]\n\n").into_bytes()]).await;
        let f = Fixture::configured(&peer, false, super::super::test_support::BASE, KEY,
            json!({"sap_orchestration_model":"gpt-4o","sap_orchestration_image_input":true})).await;
        let content = json!([{"type":"text","text":"literal {{?not_a_variable}}"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,iVBORw0KGgo=","detail":"high"}},
            {"type":"image_url","image_url":{"url":"data:image/jpeg;base64,/9j/"}}]);
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut invalid = content.clone();
            invalid[1]["image_url"]["url"] = json!("https://example.invalid/image.png");
            for invalid in [invalid, json!(vec![content[1].clone(); 11]),
                json!([{"type":"image_url","image_url":{"url":"data:image/png;base64,%%%%"}}])] {
                let response = f.open("chat/completions", json!({"model":MODEL,
                    "messages":[{"role":"user","content":invalid}]})).await;
                assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            }
            assert!(peer.oauth.lock().unwrap().is_empty());
            assert!(peer.calls.lock().unwrap().is_empty());
            for stream in [false, true] {
                let mut request = json!({"model":MODEL,"stream":stream,"messages":[{"role":"user","content":content}]});
                if stream {
                    request["tools"] = json!([{"type":"function","function":{"name":"record","parameters":{"type":"object"}}}]);
                }
                let response = f.open("chat/completions", request).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains(MODEL) && !text.contains("tenant-internal-model"), "{text}");
                if stream { assert_eq!(text.matches("[DONE]").count(), 1); }
            }
            let audits = f.audits(5).await;
            let completed: Vec<_> = audits.iter().filter(|row| row.status == "completed").collect();
            assert_eq!(completed.len(), 2);
            for row in completed { assert_identity(row); assert_eq!(row.total_tokens, Some(5)); }
            assert_eq!(f.remaining().await, 0);
        }).await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        {
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].2["messages_history"][0]["content"], content);
            let sent = &calls[1].2;
            let templating = &sent["config"]["modules"]["prompt_templating"];
            assert_eq!(templating["model"]["name"], "gpt-4o");
            assert_eq!(templating["prompt"]["template"][0]["content"], json!([
                {"type":"text","text":"{{?gateway_part_0}}"},
                {"type":"image_url","image_url":{"url":"{{?gateway_part_1}}","detail":"high"}},
                {"type":"image_url","image_url":{"url":"{{?gateway_part_2}}"}}]));
            assert_eq!(sent["placeholder_values"], json!({"gateway_part_0":content[0]["text"],
                "gateway_part_1":content[1]["image_url"]["url"],"gateway_part_2":content[2]["image_url"]["url"]}));
        }
        f.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_orchestration_signed_reasoning_round_trips_json_and_sse() {
    Box::pin(async {
        let full = json!([{"content":"visible summary"},{"signature":"opaque+/=signature"}]);
        let mut buffered = reply();
        buffered["final_result"]["choices"][0]["message"]["reasoning_content"] = full.clone();
        let mut stream = String::new();
        for block in full.as_array().unwrap() {
            let frame = stream_frame("", "", Value::Null);
            let mut frame: Value =
                serde_json::from_str(frame.trim().strip_prefix("data: ").unwrap()).unwrap();
            frame["final_result"]["choices"][0]["delta"]["reasoning_content"] = json!([block]);
            stream.push_str(&format!("data: {frame}\n\n"));
        }
        stream.push_str(&stream_frame(
            "follow-up",
            "stop",
            json!({"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}),
        ));
        stream.push_str("data: [DONE]\n\n");
        let peer =
            peer::Peer::raw_sequence(vec![buffered.to_string().into_bytes(), stream.into_bytes()])
                .await;
        let f = Fixture::configured(
            &peer,
            false,
            super::super::test_support::BASE,
            KEY,
            json!({"sap_orchestration_model":"gpt-5","sap_orchestration_reasoning":true}),
        )
        .await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut first = body("chat/completions", false);
            first["reasoning_effort"] = json!("max");
            let rejected = f.open("chat/completions", first.clone()).await;
            assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
            assert!(drain(rejected.into_body())
                .await
                .contains("invalid_sap_aicore"));
            assert!(peer.oauth.lock().unwrap().is_empty());
            first["reasoning_effort"] = json!("high");
            let resp = f.open("chat/completions", first.clone()).await;
            let status = resp.status();
            let text = drain(resp.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            let parsed: Value = serde_json::from_str(&text).unwrap();
            let assistant = &parsed["choices"][0]["message"];
            assert_eq!(parsed["model"], MODEL);
            assert_eq!(assistant["reasoning_content"], full);
            assert!(!text.contains("not-public") && !text.contains("tenant-internal-model"));
            first["messages"].as_array_mut().unwrap().extend([
                assistant.clone(),
                json!({"role":"user","content":"continue"}),
            ]);
            first["stream"] = json!(true);
            first["reasoning_effort"] = json!("xhigh");
            let resp = f.open("chat/completions", first).await;
            let status = resp.status();
            let text = drain(resp.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            assert!(
                !text.contains("synthetic-private-stage")
                    && !text.contains("tenant-internal-model")
            );
            let streamed: Vec<Value> = text
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .filter(|data| *data != "[DONE]")
                .map(|data| serde_json::from_str::<Value>(data).unwrap())
                .filter_map(|frame| {
                    frame["choices"][0]["delta"]["reasoning_content"]
                        .as_array()
                        .cloned()
                })
                .flatten()
                .collect();
            assert_eq!(Value::Array(streamed), full, "{text}");
            assert_eq!(text.matches("[DONE]").count(), 1);
            let rows = f.audits(3).await;
            assert_eq!(
                rows.iter()
                    .filter(|row| row.status == "completed" && row.total_tokens == Some(5))
                    .count(),
                2
            );
            assert_eq!(f.remaining().await, 0);
        })
        .await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        {
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 2);
            for ((_, _, request), effort) in calls.iter().zip(["high", "xhigh"]) {
                assert_eq!(
                    request["config"]["modules"]["prompt_templating"]["model"],
                    json!({"name":"gpt-5","params":{"reasoning_effort":effort}})
                );
            }
            assert_eq!(calls[1].2["messages_history"][1]["reasoning_content"], full);
        }
        f.finish().await;
        peer.stop().await;
    })
    .await;
}
