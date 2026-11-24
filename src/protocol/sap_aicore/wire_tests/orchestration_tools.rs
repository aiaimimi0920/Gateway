//! Public Chat round trips through native SAP Orchestration function tools.
use super::*;
use orchestration::fixture;

pub(super) const ARGS: &str = r#"{"city":"上海","literal":"&quot;"}"#;
const USER: &str = "literal {{?customer_value}}";
pub(super) const RESULT: &str = "结果 {{?model}}";

pub(super) fn request(stream: bool) -> Value {
    json!({"model":MODEL,"stream":stream,"messages":[{"role":"user","content":USER}],"tools":[{
        "type":"function","function":{"name":"lookup","description":"本地夹具查询","strict":true,
        "parameters":{"type":"object","properties":{"city":{"type":"string"},"literal":{"type":"string"}},
            "required":["city","literal"],"additionalProperties":false}}}]})
}

pub(super) fn call() -> Value {
    json!({"id":"call_sap","type":"function","function":{"name":"lookup","arguments":ARGS}})
}

fn usage() -> Value {
    json!({"prompt_tokens":2,"completion_tokens":3,"total_tokens":5})
}

fn envelope(call_turn: bool, streaming: bool, choice: Value, usage: Value) -> Value {
    json!({"request_id":if call_turn {"req-tools"} else {"req-result"},
        "intermediate_results":{"llm":{"private":"hidden-module"}},
        "final_result":{"id":if call_turn {"orch-tools"} else {"orch-result"},"created":123,
        "object":if streaming {"chat.completion.chunk"} else {"chat.completion"},
        "model":"tenant-internal-model","choices":[choice],"usage":usage}})
}

fn chunk(call_turn: bool, delta: Value, finish: Value, usage: Value) -> String {
    let frame = envelope(
        call_turn,
        true,
        json!({"index":0,"delta":delta,"finish_reason":finish}),
        usage,
    );
    format!("data: {frame}\n\n")
}

pub(super) fn reply(call_turn: bool, stream: bool) -> Vec<u8> {
    let message = if call_turn {
        json!({"role":"assistant","tool_calls":[call()]})
    } else {
        json!({"role":"assistant","content":"已收到结果"})
    };
    let finish = if call_turn { "tool_calls" } else { "stop" };
    if !stream {
        return envelope(
            call_turn,
            false,
            json!({"index":0,"message":message,"finish_reason":finish}),
            usage(),
        )
        .to_string()
        .into_bytes();
    }
    let (first, second) = if call_turn {
        let (prefix, suffix) = ARGS.split_at(ARGS.find(",").unwrap());
        (
            json!({"role":"assistant","content":"","tool_calls":[{"index":0,"id":"call_sap","type":"function","function":{"name":"lookup","arguments":prefix}}]}),
            json!({"content":"","tool_calls":[{"index":0,"function":{"arguments":suffix}}]}),
        )
    } else {
        (
            json!({"role":"assistant","content":"已收到"}),
            json!({"content":"结果"}),
        )
    };
    format!(
        "{}{}{}data: [DONE]\n\n",
        chunk(call_turn, first, Value::Null, Value::Null),
        chunk(call_turn, second, Value::Null, Value::Null),
        chunk(call_turn, json!({"content":""}), json!(finish), usage())
    )
    .into_bytes()
}

pub(super) fn frames(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|d| *d != "[DONE]")
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}

#[tokio::test]
async fn sap_aicore_orchestration_tools_json_sse_two_round_trip() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = peer::Peer::raw_sequence(vec![reply(true, stream), reply(false, stream)]).await;
            let f = fixture(&peer, false).await;
            let first = request(stream);
            let mut second = first.clone();
            second["messages"].as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":null,"tool_calls":[call()]}),
                json!({"role":"tool","tool_call_id":"call_sap","content":RESULT}),
            ]);
            super::super::test_support::origin(peer.origin.clone(), async {
                for (turn, input) in [first.clone(), second.clone()].into_iter().enumerate() {
                    let res = f.open("chat/completions", input).await;
                    let status = res.status();
                    let text = drain(res.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "stream={stream} turn={turn}: {text}");
                    assert!(!text.contains("tenant-internal-model") && !text.contains("hidden-module"));
                    if stream {
                        let frames = frames(&text);
                        assert!(frames.iter().all(|v| v["model"] == MODEL), "{text}");
                        assert_eq!(text.matches("[DONE]").count(), 1, "{text}");
                        assert_eq!(frames.iter().filter(|f| f["usage"]["total_tokens"] == 5).count(), 1);
                        if turn == 0 {
                            let call = frames.iter().find_map(|f| f.pointer("/choices/0/delta/tool_calls/0/id"));
                            assert_eq!(call, Some(&json!("call_sap")));
                            let arguments: String = frames.iter().filter_map(|f| f.pointer("/choices/0/delta/tool_calls/0/function/arguments").and_then(Value::as_str)).collect();
                            assert_eq!(arguments, ARGS);
                            assert!(frames.iter().any(|f| f["choices"][0]["finish_reason"] == "tool_calls"));
                        } else {
                            let content: String = frames.iter().filter_map(|f| f.pointer("/choices/0/delta/content").and_then(Value::as_str)).collect();
                            assert_eq!(content, "已收到结果");
                        }
                    } else {
                        let reply: Value = serde_json::from_str(&text).unwrap();
                        assert_eq!(reply["model"], MODEL);
                        let choice = &reply["choices"][0];
                        if turn == 0 {
                            assert_eq!(choice["finish_reason"], "tool_calls");
                            assert_eq!(choice["message"]["tool_calls"], json!([call()]));
                        } else {
                            assert_eq!(choice["message"]["content"], "已收到结果");
                        }
                    }
                }
                for audit in f.audits(2).await {
                    assert_identity(&audit);
                    assert_eq!(audit.status, "completed");
                    assert_eq!(audit.total_tokens, Some(5));
                }
                assert_eq!(f.remaining().await, 0);
            }).await;
            assert_eq!(peer.oauth.lock().unwrap().len(), 1);
            {
                let sent = peer.calls.lock().unwrap();
                assert_eq!(sent.len(), 2);
                for (path, headers, body) in sent.iter() {
                    assert_eq!(path, "/v2/inference/deployments/deployment1/v2/completion");
                    assert_eq!(headers["AI-Resource-Group"], "group-a");
                    assert_eq!(body["config"]["modules"]["prompt_templating"]["model"]["name"], "gpt-4.1-mini");
                    assert_eq!(body["config"]["modules"]["prompt_templating"]["prompt"]["tools"], first["tools"]);
                    assert!(body.get("tools").is_none());
                }
                assert_eq!(sent[0].2["placeholder_values"]["gateway_content"], USER);
                let followup = &sent[1].2;
                assert_eq!(followup["messages_history"], json!([{"role":"user","content":USER},{"role":"assistant","tool_calls":[call()]}]));
                assert_eq!(followup["placeholder_values"]["gateway_content"], RESULT);
                assert_eq!(followup["config"]["modules"]["prompt_templating"]["prompt"]["template"],
                    json!([{"role":"tool","tool_call_id":"call_sap","content":"{{?gateway_content}}"}]));
            }
            f.finish().await;
            peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn sap_aicore_orchestration_tools_reject_invalid_requests_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::raw(reply(false, false)).await;
        let f = fixture(&peer, false).await;
        let mut invalid = Vec::new();
        for (key, value) in [
            ("tools", json!([{"type":"custom","custom":{"name":"lookup"}}])),
            ("tools", json!([{"type":"function","function":{"name":"bad name"}}])),
            ("tool_choice", json!("required")),
            ("parallel_tool_calls", json!(true)),
            ("messages", json!([{"role":"user","content":USER},{"role":"tool","tool_call_id":"unmatched","content":RESULT}])),
            ("messages", json!([{"role":"user","content":USER},{"role":"assistant","tool_calls":[call()]}])),
        ] {
            let mut body = request(false);
            body[key] = value;
            invalid.push(body);
        }
        super::super::test_support::origin(peer.origin.clone(), async {
            for body in invalid {
                let res = f.open("chat/completions", body).await;
                let status = res.status();
                let text = drain(res.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
                assert!(text.contains("invalid_sap_aicore"), "{text}");
            }
            assert_eq!(f.remaining().await, 2);
        }).await;
        assert!(peer.oauth.lock().unwrap().is_empty());
        assert!(peer.calls.lock().unwrap().is_empty());
        f.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_orchestration_tools_missing_stream_identity_refunds_without_replay() {
    Box::pin(async {
        let first = chunk(
            true,
            json!({"content":"","tool_calls":[{"index":0,"function":{"arguments":ARGS}}]}),
            Value::Null,
            Value::Null,
        );
        let last = chunk(true, json!({"content":""}), json!("tool_calls"), usage());
        let peer = peer::Peer::raw(format!("{first}{last}data: [DONE]\n\n").into_bytes()).await;
        let f = fixture(&peer, true).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut body = f.open("chat/completions", request(true)).await.into_body();
            let mut received = Vec::new();
            tokio::time::timeout(Duration::from_secs(5), async {
                while let Some(Ok(frame)) = body.frame().await {
                    if let Ok(bytes) = frame.into_data() {
                        received.extend_from_slice(&bytes);
                    }
                }
            })
            .await
            .unwrap();
            drop(body);
            let text = String::from_utf8(received).unwrap();
            assert!(
                !text.contains("[DONE]")
                    && !text.contains("total_tokens")
                    && !text.contains("\"finish_reason\":\"tool_calls\""),
                "{text}"
            );
            assert_ne!(f.audits(1).await[0].status, "completed");
            assert_eq!(f.remaining().await, 2);
            assert_eq!(peer.oauth.lock().unwrap().len(), 1);
            assert_eq!(peer.calls.lock().unwrap().len(), 1);
        })
        .await;
        f.finish().await;
        peer.stop().await;
    })
    .await;
}
