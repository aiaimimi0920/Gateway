//! Full Vertex's typed OAuth fixture owns target/auth/audit; only public wire readers are reused.
use super::*;
#[path = "../vertex_express_pipeline_tool_wire.rs"]
pub(super) mod public_wire;

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

pub(super) fn assert_roundtrip(server: &wire::Server, calls: &[public_wire::Call]) {
    assert_wire(server, 2);
    let captured = server.calls.lock().unwrap();
    assert_eq!(
        captured.len(),
        2,
        "one generation per request; never replay unknown consumption"
    );
    let first = &captured[0].2;
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
    let contents = captured[1].2["contents"].as_array().unwrap();
    let groups: Vec<_> = contents
        .iter()
        .filter(|c| {
            c["parts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p.get("functionResponse").is_some())
        })
        .collect();
    assert_eq!(
        groups.len(),
        1,
        "v1 parallel results share one user Content"
    );
    assert_eq!(groups[0]["role"], "user");
    let functions: Vec<_> = contents
        .iter()
        .flat_map(|c| c["parts"].as_array().unwrap())
        .filter_map(|p| p.get("functionCall"))
        .collect();
    let results = groups[0]["parts"].as_array().unwrap();
    assert_eq!(functions.len(), calls.len());
    assert_eq!(results.len(), calls.len());
    for (i, call) in calls.iter().enumerate() {
        assert!(functions[i].get("id").is_none());
        assert_eq!(functions[i]["name"], call.name);
        assert_eq!(functions[i]["args"], call.args);
        assert!(results[i]["functionResponse"].get("id").is_none());
        assert_eq!(results[i]["functionResponse"]["name"], call.name);
        assert_eq!(
            results[i]["functionResponse"]["response"],
            json!({"ordinal":i,"city":"杭州"})
        );
    }
    assert!(captured[1].2.get("gateway_vertex_history").is_none());
}

#[tokio::test]
async fn vertex_oauth_tools_three_protocols_json_sse_two_turns_and_local_settlement() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let part = json!({"functionCall":{"name":"lookup","args":{"city":"杭州","nested":[1,2]}}});
                let raw = event(json!({"candidates":[{"content":{"parts":[part,part]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}}));
                let server = wire::Server::sequence(vec![raw,success_body()]).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, public_wire::request_body(endpoint,stream)).await;
                    let status = response.status(); let text = drain(response.into_body()).await;
                    assert_eq!(status,StatusCode::OK,"{text}");
                    let calls = public_wire::calls(&text,endpoint,stream);
                    assert_eq!(calls.len(),2); assert_ne!(calls[0].id,calls[1].id);
                    for call in &calls { assert!(call.id.starts_with("call_vertex_oauth_")); assert_eq!(call.args,json!({"city":"杭州","nested":[1,2]})); }
                    assert_eq!(fixture.audit().await.status,"completed"); assert_eq!(fixture.remaining().await,1);
                    let response = fixture.open(endpoint, public_wire::followup(endpoint,stream,&calls)).await;
                    let status = response.status(); let text = drain(response.into_body()).await;
                    assert_eq!(status,StatusCode::OK,"{text}"); assert!(text.contains("合成答案") && !text.contains(ACCESS));
                    for row in audits(&fixture,2).await { assert_identity(&row); assert_eq!(row.status,"completed"); assert_eq!(row.total_tokens,Some(5)); }
                    assert_eq!(fixture.remaining().await,0); assert_roundtrip(&server,&calls);
                }).await;
                fixture.finish().await; server.stop().await;
            }
        }
    }).await;
}

#[tokio::test]
async fn vertex_oauth_tools_unknown_result_is_rejected_before_network_and_refunded() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::start(200, success_body()).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                let calls = vec![public_wire::Call {
                    id: "known".into(),
                    name: "lookup".into(),
                    args: json!({}),
                }];
                let mut req = public_wire::followup(endpoint, stream, &calls);
                match endpoint {
                    "messages" => {
                        req["messages"][2]["content"][0]["tool_use_id"] = json!("unknown")
                    }
                    "responses" => req["input"][2]["call_id"] = json!("unknown"),
                    _ => req["messages"][2]["tool_call_id"] = json!("unknown"),
                };
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, req).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
                    assert!(
                        text.contains("vertex_oauth_invalid_tool_history")
                            && !text.contains(ACCESS)
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
