//! Public prepaid Chat through native SAP Bedrock, including caller-owned continuations.
use super::*;

const ID: &str = "tooluse.sap:1";
const ARGUMENTS: &str = r#"{"city":"上海 &quot;"}"#;
const RESULT: &str = r#"{"temperature":21}"#;

fn request(stream: bool) -> Value {
    let mut req = body("chat/completions", stream);
    req["tools"] = json!([{"type":"function","function":{"name":"weather","strict":true,
        "parameters":{"type":"object","properties":{"city":{"type":"string"}},
            "required":["city"],"additionalProperties":false}}}]);
    req["tool_choice"] = json!("auto");
    req
}

fn follow_up(mut req: Value, call: Value) -> Value {
    req["messages"].as_array_mut().unwrap().extend([
        json!({"role":"assistant","tool_calls":[call]}),
        json!({"role":"tool","tool_call_id":ID,"content":RESULT}),
    ]);
    req
}

fn chat_call(arguments: &str) -> Value {
    json!({"id":ID,"type":"function","function":{"name":"weather","arguments":arguments}})
}

fn native_reply(content: Value, reason: &str) -> Vec<u8> {
    json!({"model":"tenant-private-model","output":{"message":{"role":"assistant","content":content}},
        "stopReason":reason,"usage":{"inputTokens":2,"outputTokens":3,"totalTokens":5}})
    .to_string().into_bytes()
}

fn native_call(id: &str, name: &str, input: Value) -> Value {
    json!({"toolUse":{"toolUseId":id,"name":name,"input":input}})
}

fn tool_events(id: &str, name: &str, input: &str, stop: &str) -> Vec<u8> {
    let mid = input.find(':').unwrap_or(0);
    let (prefix, suffix) = input.split_at(mid);
    [
        event("messageStart", json!({"role":"assistant"})),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":"查询"}}),
        ),
        event("contentBlockStop", json!({"contentBlockIndex":0})),
        event(
            "contentBlockStart",
            json!({"contentBlockIndex":1,"start":{"toolUse":{"toolUseId":id,"name":name}}}),
        ),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":prefix}}}),
        ),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":suffix}}}),
        ),
        event("contentBlockStop", json!({"contentBlockIndex":1})),
        event("messageStop", json!({"stopReason":stop})),
        event(
            "metadata",
            json!({"usage":{"inputTokens":2,"outputTokens":3,"totalTokens":5}}),
        ),
    ]
    .concat()
}

fn parse_turn(text: &str, stream: bool, first: bool) -> Option<Value> {
    assert!(
        !text.contains("tenant-private-model") && !text.contains("gateway_bedrock"),
        "{text}"
    );
    let finish = if first { "tool_calls" } else { "stop" };
    if !stream {
        let res: Value = serde_json::from_str(text).unwrap();
        assert_eq!(res["model"], MODEL);
        assert_eq!(res["usage"]["total_tokens"], 5);
        assert_eq!(res["choices"][0]["finish_reason"], finish);
        if first {
            let call = &res["choices"][0]["message"]["tool_calls"][0];
            assert_eq!(call, &chat_call(ARGUMENTS));
            return Some(call.clone());
        }
        assert_eq!(res["choices"][0]["message"]["content"], "合成回复");
        return None;
    }
    assert_eq!(text.matches("data: [DONE]").count(), 1, "{text}");
    let chunks: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|data| *data != "[DONE]")
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    assert!(chunks.iter().all(|v| v["model"] == MODEL), "{text}");
    assert_eq!(
        chunks.last().unwrap()["choices"][0]["finish_reason"],
        finish
    );
    assert_eq!(
        chunks
            .iter()
            .filter(|v| v["usage"]["total_tokens"] == 5)
            .count(),
        1
    );
    if !first {
        let answer: String = chunks
            .iter()
            .filter_map(|v| v.pointer("/choices/0/delta/content")?.as_str())
            .collect();
        assert_eq!(answer, "合成回复");
        return None;
    }
    let calls: Vec<_> = chunks
        .iter()
        .filter_map(|v| v.pointer("/choices/0/delta/tool_calls/0"))
        .collect();
    assert!(calls.iter().all(|call| call["index"] == 0));
    assert_eq!(calls[0]["type"], "function");
    let arguments: String = calls
        .iter()
        .filter_map(|call| call.pointer("/function/arguments")?.as_str())
        .collect();
    assert_eq!(arguments, ARGUMENTS);
    let call = json!({"id":calls[0]["id"],"type":"function","function":{"name":calls[0]["function"]["name"],"arguments":arguments}});
    assert_eq!(call, chat_call(ARGUMENTS));
    Some(call)
}

#[tokio::test]
async fn sap_aicore_bedrock_tools_round_trip() {
    Box::pin(async {
        let args: Value = serde_json::from_str(ARGUMENTS).unwrap();
        for stream in [false, true] {
            let first = if stream { tool_events(ID, "weather", ARGUMENTS, "tool_use") }
                else { native_reply(json!([native_call(ID,"weather",args.clone())]), "tool_use") };
            let peer = peer::Peer::raw_sequence(vec![first, if stream { events() } else { reply() }]).await;
            let f = fixture(&peer, false).await;
            super::super::super::test_support::origin(peer.origin.clone(), async {
                let req = request(stream);
                let res = f.open("chat/completions", req.clone()).await;
                let status = res.status();
                let text = drain(res.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                let call = parse_turn(&text, stream, true).unwrap();
                let res = f.open("chat/completions", follow_up(req, call)).await;
                let status = res.status();
                let text = drain(res.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                parse_turn(&text, stream, false);
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
                for (path, _, wire) in sent.iter() {
                    let op = if stream { "converse-stream" } else { "converse" };
                    assert_eq!(path, &format!("/v2/inference/deployments/deployment1/{op}"));
                    assert_eq!(wire["toolConfig"], json!({"toolChoice":{"auto":{}},"tools":[{"toolSpec":{
                        "name":"weather","strict":true,"inputSchema":{"json":request(stream)["tools"][0]["function"]["parameters"]}}}]}));
                }
                assert_eq!(sent[1].2["messages"], json!([
                    {"role":"user","content":[{"text":"合成问题"}]},
                    {"role":"assistant","content":[native_call(ID,"weather",args.clone())]},
                    {"role":"user","content":[{"toolResult":{"toolUseId":ID,"content":[{"text":RESULT}]}}]},
                ]));
            }
            f.finish().await;
            peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn sap_aicore_bedrock_tools_reject_ambiguous_history_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::raw(reply()).await;
        let f = fixture(&peer, false).await;
        let mut cases = Vec::new();
        for arguments in ["", "[]", "{", r#"{"x":1,"x":2}"#] {
            cases.push(follow_up(request(false), chat_call(arguments)));
        }
        for (pointer, replacement) in [
            ("/messages/2/tool_call_id", json!("not-pending")),
            (
                "/messages/1/tool_calls/0/function/name",
                json!("not_declared"),
            ),
            ("/tool_choice", json!("none")),
            (
                "/tool_choice",
                json!({"type":"function","function":{"name":"not_declared"}}),
            ),
            ("/tools/0/function/strict", Value::Null),
            ("/tools/0/function/parameters/type", json!("array")),
        ] {
            let mut req = follow_up(request(false), chat_call(ARGUMENTS));
            *req.pointer_mut(pointer).unwrap() = replacement;
            cases.push(req);
        }
        let mut req = follow_up(request(false), chat_call(ARGUMENTS));
        req["messages"][1]["tool_calls"]
            .as_array_mut()
            .unwrap()
            .push(chat_call(ARGUMENTS));
        cases.push(req);
        let mut req = follow_up(request(false), chat_call(ARGUMENTS));
        req["messages"].as_array_mut().unwrap().pop();
        cases.push(req);
        let mut req = follow_up(request(false), chat_call(ARGUMENTS));
        let duplicate = req["messages"][2].clone();
        req["messages"].as_array_mut().unwrap().push(duplicate);
        cases.push(req);
        super::super::super::test_support::origin(peer.origin.clone(), async {
            for req in cases {
                let res = f.open("chat/completions", req).await;
                let status = res.status();
                let text = drain(res.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
            }
            assert_eq!(f.remaining().await, 2);
        })
        .await;
        assert!(peer.oauth.lock().unwrap().is_empty() && peer.calls.lock().unwrap().is_empty());
        f.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_bedrock_tools_reject_invalid_results_and_refund_without_replay() {
    Box::pin(async {
        let args = json!({"city":"上海"});
        let call = native_call(ID, "weather", args.clone());
        let cases = [
            (
                "undeclared",
                false,
                false,
                native_reply(json!([native_call(ID, "not_declared", args)]), "tool_use"),
            ),
            (
                "duplicate ID",
                false,
                false,
                native_reply(json!([call.clone(), call]), "tool_use"),
            ),
            (
                "duplicate JSON key",
                true,
                false,
                tool_events(
                    ID,
                    "weather",
                    r#"{"city":"上海","city":"北京"}"#,
                    "tool_use",
                ),
            ),
            (
                "non-object input",
                true,
                false,
                tool_events(ID, "weather", "[]", "tool_use"),
            ),
            (
                "contradictory finish",
                true,
                false,
                tool_events(ID, "weather", ARGUMENTS, "end_turn"),
            ),
            (
                "reused history ID",
                true,
                true,
                tool_events(ID, "weather", ARGUMENTS, "tool_use"),
            ),
        ];
        for (case, stream, history, response) in cases {
            // An expected upstream error opens the real circuit. Isolate cases so each one
            // actually reaches the transport rather than being blocked by the previous case.
            let peer = peer::Peer::raw(response).await;
            let f = fixture(&peer, true).await;
            super::super::super::test_support::origin(peer.origin.clone(), async {
                let mut req = request(stream);
                if history {
                    req = follow_up(req, chat_call(ARGUMENTS));
                }
                let res = f.open("chat/completions", req).await;
                if stream && res.status() == StatusCode::OK {
                    let mut body = res.into_body();
                    let (bytes, failed) = tokio::time::timeout(Duration::from_secs(5), async {
                        let mut bytes = Vec::new();
                        while let Some(frame) = body.frame().await {
                            match frame {
                                Ok(frame) => {
                                    if let Ok(data) = frame.into_data() {
                                        bytes.extend_from_slice(&data);
                                    }
                                }
                                Err(_) => return (bytes, true),
                            }
                        }
                        (bytes, false)
                    })
                    .await
                    .unwrap();
                    assert!(
                        failed,
                        "{case}: invalid native stream produced a successful body"
                    );
                    let text = String::from_utf8(bytes).unwrap();
                    assert!(
                        !text.contains("[DONE]") && !text.contains("total_tokens"),
                        "{case}: {text}"
                    );
                    drop(body);
                } else {
                    let status = res.status();
                    let text = drain(res.into_body()).await;
                    assert_eq!(status, StatusCode::BAD_GATEWAY, "{case}: {text}");
                    assert!(
                        !text.contains("tenant-private-model") && !text.contains("not_declared")
                    );
                }
                assert_ne!(f.audits(1).await[0].status, "completed", "{case}");
                assert_eq!(f.remaining().await, 2, "{case}");
            })
            .await;
            assert_eq!(peer.oauth.lock().unwrap().len(), 1, "{case}");
            assert_eq!(peer.calls.lock().unwrap().len(), 1, "{case}");
            f.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_bedrock_tools_cancel_before_eof_refunds_and_drops_upstream() {
    Box::pin(async {
        let peer = peer::Peer::raw_held(tool_events(ID, "weather", ARGUMENTS, "tool_use")).await;
        let f = fixture(&peer, true).await;
        super::super::super::test_support::origin(peer.origin.clone(), async {
            let res = f.open("chat/completions", request(true)).await;
            assert_eq!(res.status(), StatusCode::OK);
            let mut body = res.into_body();
            let mut received = Vec::new();
            tokio::time::timeout(Duration::from_secs(3), async {
                while let Some(frame) = body.frame().await {
                    if let Ok(bytes) = frame.unwrap().into_data() {
                        received.extend_from_slice(&bytes);
                    }
                    if String::from_utf8_lossy(&received).contains(ID) {
                        return;
                    }
                }
                panic!("tool delta not delivered before native EOF");
            })
            .await
            .unwrap();
            let text = String::from_utf8(received).unwrap();
            assert!(
                !text.contains("[DONE]") && !text.contains("total_tokens"),
                "{text}"
            );
            drop(body);
            peer.wait_closed().await;
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

#[test]
fn sap_aicore_bedrock_tools_maps_explicit_choices_without_loss() {
    for (choice, expected) in [
        (json!("required"), json!({"any":{}})),
        (
            json!({"type":"function","function":{"name":"weather"}}),
            json!({"tool":{"name":"weather"}}),
        ),
    ] {
        let mut value = request(false);
        value["tool_choice"] = choice;
        let req = crate::protocol::openai::normalize_chat_completions(value).unwrap();
        let body =
            super::super::super::bedrock_tools::pack(&req, UPSTREAM_MODEL, false, false).unwrap();
        assert_eq!(body["toolConfig"]["toolChoice"], expected);
    }
}
