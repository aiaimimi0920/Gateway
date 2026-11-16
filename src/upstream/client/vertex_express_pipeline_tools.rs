//! Three real public protocols, normal Key/SQLite settlement, and two-turn tool/result exchange.
use super::*;
#[path = "vertex_express_pipeline_tool_wire.rs"]
pub(super) mod public_wire;

fn tool_body(reason: Option<&str>) -> Vec<u8> {
    let part = json!({"functionCall":{"name":"lookup","args":{"city":"杭州","nested":[1,2]}}});
    let first = event(json!({"candidates":[{"content":{"parts":[part,part]}}]}));
    let last = event(
        json!({"candidates":[{"finishReason":reason.unwrap_or("STOP")}],
        "usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}}),
    );
    [first, last].concat()
}

pub(super) async fn audits(fixture: &Fixture, count: usize) -> Vec<db::GatewayRequestAuditView> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = fixture
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&db::RequestAuditFilters::default())
                .await
                .unwrap();
            if rows.len() == count
                && rows
                    .iter()
                    .all(|r| !matches!(r.status.as_str(), "started" | "running"))
            {
                while Arc::strong_count(&fixture.state) > 1 {
                    tokio::task::yield_now().await;
                }
                return rows;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

pub(super) fn assert_roundtrip_wire(server: &wire::Server, tools: &[public_wire::Call]) {
    assert_wire_attempts(server, 2);
    let calls = server.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "one generation per public request, no replay"
    );
    let first = &calls[0].2;
    assert_eq!(
        first["tools"][0]["functionDeclarations"][0]["name"],
        "lookup"
    );
    assert_eq!(
        first["tools"][0]["functionDeclarations"][0]["parameters"],
        public_wire::schema()
    );
    assert_eq!(first["toolConfig"]["functionCallingConfig"]["mode"], "ANY");
    assert_eq!(
        first["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"],
        json!(["lookup"])
    );
    let parts: Vec<_> = calls[1].2["contents"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["parts"].as_array().unwrap())
        .collect();
    let history: Vec<_> = parts.iter().filter_map(|p| p.get("functionCall")).collect();
    let results: Vec<_> = parts
        .iter()
        .filter_map(|p| p.get("functionResponse"))
        .collect();
    assert_eq!(history.len(), 2);
    assert_eq!(results.len(), 2);
    let result_contents: Vec<_> = calls[1].2["contents"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|content| {
            content["parts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|part| part.get("functionResponse").is_some())
        })
        .collect();
    assert_eq!(
        result_contents.len(),
        1,
        "all parallel v1 results must share one Content"
    );
    for (i, tool) in tools.iter().enumerate() {
        assert!(
            history[i].get("id").is_none(),
            "v1 does not carry functionCall.id"
        );
        assert!(
            results[i].get("id").is_none(),
            "v1 does not carry functionResponse.id"
        );
        assert_eq!(history[i]["args"], tool.args);
        assert_eq!(results[i]["name"], tool.name);
        assert_eq!(results[i]["response"], json!({"ordinal":i,"city":"杭州"}));
    }
}

#[tokio::test]
async fn vertex_express_pipeline_tools_three_protocols_json_and_sse_roundtrip() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::sequence(vec![tool_body(None), success_body()]).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture
                        .open(endpoint, public_wire::request_body(endpoint, stream))
                        .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let text = drain(response.into_body()).await;
                    assert!(!text.contains(KEY), "{text}");
                    let tools = public_wire::calls(&text, endpoint, stream);
                    assert_eq!(tools.len(), 2, "{endpoint} stream={stream}: {text}");
                    assert!(!tools[0].id.is_empty() && !tools[1].id.is_empty());
                    assert_ne!(tools[0].id, tools[1].id);
                    for tool in &tools {
                        assert_eq!(tool.name, "lookup");
                        assert_eq!(tool.args, json!({"city":"杭州","nested":[1,2]}));
                    }
                    assert_eq!(fixture.audit().await.status, "completed");
                    assert_eq!(fixture.remaining().await, 1);
                    let response = fixture
                        .open(endpoint, public_wire::followup(endpoint, stream, &tools))
                        .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let text = drain(response.into_body()).await;
                    assert!(text.contains("合成答案") && !text.contains(KEY), "{text}");
                    for row in audits(&fixture, 2).await {
                        assert_eq!(row.status, "completed");
                        assert_eq!(
                            (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                            (Some(2), Some(3), Some(5))
                        );
                        assert_identity(&row);
                    }
                    assert_eq!(fixture.remaining().await, 0);
                    assert_roundtrip_wire(&server, &tools);
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}

#[tokio::test]
async fn vertex_express_pipeline_tools_filtered_candidate_is_not_executable_or_natural_stop() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let mut body = tool_body(Some("RECITATION"));
                let first = event(
                    json!({"candidates":[{"content":{"parts":[{"text":"private-blocked-text"}]}}]}),
                );
                body.splice(0..0, first);
                let server = wire::Server::start(200, body).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture
                        .open(endpoint, public_wire::request_body(endpoint, stream))
                        .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let text = drain(response.into_body()).await;
                    assert!(
                        !text.contains("private-blocked-text")
                            && !text.contains("lookup")
                            && !text.contains(KEY),
                        "{text}"
                    );
                    if stream {
                        let frames = public_wire::frames(&text);
                        assert!(
                            frames.iter().any(|f| match endpoint {
                                "messages" => f["delta"]["stop_reason"] == "refusal",
                                "responses" =>
                                    f["type"] == "response.incomplete"
                                        && f["response"]["status"] == "incomplete",
                                _ => f["choices"][0]["finish_reason"] == "content_filter",
                            }),
                            "{text}"
                        );
                    } else {
                        let response: Value = serde_json::from_str(&text).unwrap();
                        assert_eq!(
                            match endpoint {
                                "messages" => &response["stop_reason"],
                                "responses" => &response["status"],
                                _ => &response["choices"][0]["finish_reason"],
                            },
                            match endpoint {
                                "messages" => "refusal",
                                "responses" => "incomplete",
                                _ => "content_filter",
                            },
                            "{text}"
                        );
                    }
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "completed");
                    assert_eq!(row.total_tokens, Some(5));
                    assert_eq!(fixture.remaining().await, 1);
                    assert_wire(&server);
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}

#[tokio::test]
async fn vertex_express_pipeline_tools_invalid_generation_never_replays_or_debits_success() {
    Box::pin(async {
        for raw in [tool_body(Some("MALFORMED_FUNCTION_CALL")),event(json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"functionCall":{"name":"lookup","args":"not-json"}}]}}]}))] {
            for endpoint in ["chat/completions", "messages", "responses"] {
                for stream in [false,true] {
                    let server = wire::Server::start(200,raw.clone()).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin, async {
                        let response = fixture.open(endpoint,public_wire::request_body(endpoint,stream)).await;
                        assert_eq!(response.status(),StatusCode::BAD_GATEWAY);
                        let text = drain(response.into_body()).await;
                        assert!(!text.contains("[DONE]") && !text.contains("response.completed") && !text.contains("message_stop") && !text.contains(KEY),"{text}");
                        assert_eq!(fixture.audit().await.status,"failed");
                        assert_eq!(fixture.remaining().await,2);
                        assert_wire(&server);
                    }).await;
                    fixture.finish().await;
                    server.stop().await;
                }
            }
        }
    }).await;
}

#[tokio::test]
async fn vertex_express_pipeline_tools_unknown_history_result_is_rejected_before_network() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::start(200, success_body()).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                let tools = vec![public_wire::Call {
                    id: "known".into(),
                    name: "lookup".into(),
                    args: json!({}),
                }];
                let mut request = public_wire::followup(endpoint, stream, &tools);
                match endpoint {
                    "messages" => {
                        request["messages"][2]["content"][0]["tool_use_id"] = json!("unknown")
                    }
                    "responses" => request["input"][2]["call_id"] = json!("unknown"),
                    _ => request["messages"][2]["tool_call_id"] = json!("unknown"),
                }
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, request).await;
                    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
                    let text = drain(response.into_body()).await;
                    assert!(
                        text.contains("vertex_express_invalid_tool_history") && !text.contains(KEY),
                        "{text}"
                    );
                    assert_eq!(fixture.audit().await.status, "failed");
                    assert_eq!(fixture.remaining().await, 2);
                    assert!(server.calls.lock().unwrap().is_empty());
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}
