//! Public Chat must return reasoning and carry it back on the original tool turn.
use super::*;

const REASONING: &str = "fixture reasoning\n  exact whitespace";
const FLASH: &str = "deepseek-flash";
const TOOL: &str = "fixture_tool_call";

fn answer(first: bool, stream: bool) -> Vec<u8> {
    let usage = json!({"prompt_tokens":2,"completion_tokens":3,"total_tokens":5,
        "completion_tokens_details":{"reasoning_tokens":2}});
    let tool = json!({"id":TOOL,"type":"function","function":{"name":"lookup","arguments":"{}"}});
    let finish = if first { "tool_calls" } else { "stop" };
    let mut message = if first {
        json!({"role":"assistant","content":null,"reasoning_content":REASONING,"tool_calls":[tool]})
    } else {
        json!({"role":"assistant","content":"fixture final answer"})
    };
    if !stream {
        return json!({"id":"chatcmpl-fixture","object":"chat.completion","model":FLASH,
            "choices":[{"index":0,"message":message,"finish_reason":finish}],"usage":usage})
        .to_string()
        .into_bytes();
    }
    if first {
        message["tool_calls"][0]["index"] = json!(0);
    }
    let chunk = |choices: Value| json!({"id":"chatcmpl-fixture","object":"chat.completion.chunk","model":FLASH,"choices":choices});
    let delta = chunk(json!([{"index":0,"delta":message,"finish_reason":null}]));
    let end = chunk(json!([{"index":0,"delta":{},"finish_reason":finish}]));
    let mut totals = chunk(json!([]));
    totals["usage"] = usage;
    format!("data: {delta}\n\ndata: {end}\n\ndata: {totals}\n\ndata: [DONE]\n\n").into_bytes()
}

#[tokio::test]
async fn sensenova_chat_reasoning_round_trips_with_tool_history_json_and_sse() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = peer::Peer::raw_then(answer(true, stream), answer(false, stream)).await;
            let document = json!({"providers":[{"id":"sensenova-public","preset":"sensenova-token-plan",
                "base_url":super::super::BASE,"explicit_credentials_only":true,
                "supported_models":[MODEL],"model_map":{MODEL:FLASH},
                "credentials":[{"id":"exact","api_key":KEY,"supported_models":[MODEL]}]}],"model_routes":[]});
            let fixture = Box::pin(Fixture::with_document(&peer, document, 2)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let mut request = body("chat/completions", stream);
                request["reasoning_effort"] = json!("high");
                request["tools"] = json!([{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}]);
                let first = fixture.open("chat/completions", request.clone()).await;
                let status = first.status();
                let first = drain(first.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{first}");
                let mut returned = if stream {
                    assert!(first.ends_with("data: [DONE]\n\n"), "{first}");
                    first.lines().filter_map(|line| line.strip_prefix("data: "))
                        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                        .find_map(|frame| frame["choices"][0]["delta"].as_object()
                            .filter(|delta| delta.contains_key("reasoning_content")).cloned())
                        .map(Value::Object).unwrap()
                } else {
                    let response = serde_json::from_str::<Value>(&first).unwrap();
                    assert_eq!(response["model"], MODEL);
                    response["choices"][0]["message"].clone()
                };
                assert_eq!(returned["reasoning_content"], REASONING);
                returned["tool_calls"][0].as_object_mut().unwrap().remove("index");
                request["messages"].as_array_mut().unwrap().extend([
                    returned, json!({"role":"tool","tool_call_id":TOOL,"content":"lookup result"}),
                ]);
                let second = fixture.open("chat/completions", request).await;
                let status = second.status();
                let second = drain(second.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{second}");
                assert!(second.contains("fixture final answer"), "{second}");
                let rows = fixture.audits(2).await;
                for row in &rows {
                    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
                    assert_eq!(row.resolved_model.as_deref(), Some(FLASH));
                    assert_eq!(row.route_trace.as_ref().unwrap()["realCredentialRef"], "exact");
                    assert_eq!(row.status, "completed");
                    assert_eq!((row.prompt_tokens, row.completion_tokens, row.total_tokens), (Some(2), Some(3), Some(5)));
                }
                assert_eq!(fixture.remaining().await, 0);
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), 2, "no replay");
                for (path, headers, wire) in calls.iter() {
                    assert_eq!(path, "/v1/chat/completions");
                    assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
                    assert_eq!(wire["model"], FLASH);
                }
                let followup = &calls[1].2;
                assert_eq!(followup["reasoning_effort"], "high");
                assert_eq!(followup["messages"][1]["reasoning_content"], REASONING);
                assert_eq!(followup["messages"][1]["tool_calls"][0]["id"], TOOL);
                assert_eq!(followup["messages"][2]["tool_call_id"], TOOL);
            }).await;
            fixture.finish().await;
            peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn sensenova_chat_reasoning_rejects_malformed_history_without_send_or_charge() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            for (index, (role, reasoning, stream)) in [
                ("user", json!("not an assistant turn"), false),
                ("assistant", json!({"content":"wrong type"}), false),
                ("user", json!("not an assistant turn"), true),
                ("assistant", json!(["wrong type"]), true),
            ]
            .into_iter()
            .enumerate()
            {
                let mut request = body("chat/completions", stream);
                request["messages"][0]["role"] = json!(role);
                request["messages"][0]["reasoning_content"] = reasoning;
                let response = fixture.open("chat/completions", request).await;
                let status = response.status();
                let output = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{output}");
                assert!(
                    output.contains("sensenova_chat_reasoning_history_invalid"),
                    "{output}"
                );
                let rows = fixture.audits(index + 1).await;
                assert!(rows
                    .iter()
                    .all(|row| row.status == "failed" && row.provider_account_id.is_none()));
                assert!(rows
                    .iter()
                    .all(|row| row.route_trace.as_ref().is_some_and(|trace| trace
                        ["realCredentialRef"]
                        .is_null()
                        && trace["routeAttemptCount"] == 0
                        && trace["attemptedProviderIds"] == json!([])
                        && trace["rejectedCandidates"].as_array().unwrap().iter().any(
                            |rejected| rejected["providerAccountId"] == "sensenova-public"
                                && rejected["realCredentialRef"] == "exact"
                                && rejected["resolvedModel"] == UPSTREAM_MODEL
                                && rejected["reason"] == "sensenova_chat_reasoning_history_invalid"
                        ))));
                assert!(peer.calls.lock().unwrap().is_empty());
                assert_eq!(fixture.remaining().await, 2);
            }
            // Repeated caller errors must not put a healthy credential into cooldown.
            let response = fixture
                .open("chat/completions", body("chat/completions", false))
                .await;
            let status = response.status();
            let output = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{output}");
            let rows = fixture.audits(5).await;
            assert_eq!(
                rows.iter().filter(|row| row.status == "completed").count(),
                1
            );
            assert_eq!(peer.calls.lock().unwrap().len(), 1);
            assert_eq!(fixture.remaining().await, 1);
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
