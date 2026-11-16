//! Preview opt-in crosses each normal public adapter without replacing tool-choice semantics.
use super::tools::public_wire;
use super::*;

fn preview_body() -> Vec<u8> {
    let mut body = Vec::new();
    for _ in 0..2 {
        for call in [
            json!({"name":"lookup","willContinue":true}),
            json!({"partialArgs":[{"jsonPath":"$.city","stringValue":"杭","willContinue":true}],"willContinue":true}),
            json!({"partialArgs":[{"jsonPath":"$.city","stringValue":"州"},{"jsonPath":"$.nested[0]","numberValue":1},{"jsonPath":"$.nested[1]","numberValue":2}],"willContinue":true}),
            json!({}),
        ] {
            body.extend(event(
                json!({"candidates":[{"content":{"parts":[{"functionCall":call}]}}]}),
            ));
        }
    }
    body.extend(event(json!({"candidates":[{"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}})));
    body
}

fn opt_in(body: &mut Value, value: Value) {
    body["toolConfig"] = json!({"functionCallingConfig":{"streamFunctionCallArguments":value}});
}

#[tokio::test]
async fn vertex_express_preview_opt_in_preserves_choice_across_three_protocols() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
              for enabled in [false,true] {
                let server=wire::Server::start(200,success_body()).await;
                let fixture=Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin,async {
                    let mut request=public_wire::request_body(endpoint,stream);
                    opt_in(&mut request,json!(enabled));
                    let response=fixture.open(endpoint,request).await;
                    assert_eq!(response.status(),StatusCode::OK);
                    assert!(drain(response.into_body()).await.contains("合成答案"));
                    assert_eq!(fixture.audit().await.status,"completed");
                    let calls=server.calls.lock().unwrap();
                    assert_eq!(calls.len(),1);
                    assert_eq!(calls[0].2["toolConfig"]["functionCallingConfig"],json!({"mode":"ANY","allowedFunctionNames":["lookup"],"streamFunctionCallArguments":enabled}));
                }).await;
                fixture.finish().await;server.stop().await;
              }
            }
        }
    }).await;
}

#[tokio::test]
async fn vertex_express_preview_incomplete_or_conflicting_call_is_not_successfully_charged() {
    Box::pin(async {
        let start=event(json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup","partialArgs":[{"jsonPath":"$.city","stringValue":"private-fragment","willContinue":true}],"willContinue":true}}]}}]}));
        let finish=event(json!({"candidates":[{"finishReason":"STOP"}],"usageMetadata":{"totalTokenCount":5}}));
        for tail in [Vec::new(),finish.clone(),[event(json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"other"}}]}}]})),finish].concat()] {
            for endpoint in ["chat/completions","messages","responses"] {
                for stream in [false,true] {
                    let server=wire::Server::start(200,[start.clone(),tail.clone()].concat()).await;
                    let fixture=Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin,async {
                        let mut request=public_wire::request_body(endpoint,stream);opt_in(&mut request,json!(true));
                        let response=fixture.open(endpoint,request).await;
                        assert_eq!(response.status(),StatusCode::BAD_GATEWAY);
                        let text=drain(response.into_body()).await;
                        for private in [KEY,"private-fragment","[DONE]","response.completed","message_stop"] {assert!(!text.contains(private),"{text}");}
                        assert_eq!(fixture.audit().await.status,"failed");assert_eq!(fixture.remaining().await,2);
                        assert_wire(&server);assert_eq!(server.calls.lock().unwrap().len(),1);
                    }).await;
                    fixture.finish().await;server.stop().await;
                }
            }
        }
    }).await;
}

#[tokio::test]
async fn vertex_express_preview_caller_drop_refunds_and_closes_partial_argument_source() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
                let pending=event(json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup","willContinue":true}}]}}]}));
                let server=wire::Server::stream(200,pending,true).await;
                let fixture=Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin,async {
                    let mut request=public_wire::request_body(endpoint,stream);opt_in(&mut request,json!(true));
                    let mut call=Box::pin(fixture.open(endpoint,request));
                    assert!(tokio::time::timeout(Duration::from_millis(200),call.as_mut()).await.is_err());
                    assert_wire(&server);assert_eq!(fixture.remaining().await,1);drop(call);
                    assert_eq!(fixture.audit().await.status,"cancelled");assert_eq!(fixture.remaining().await,2);
                    tokio::time::timeout(Duration::from_secs(3),async {while server.closed()==0 {tokio::task::yield_now().await;}}).await.unwrap();
                    assert_eq!(server.calls.lock().unwrap().len(),1);
                }).await;
                fixture.finish().await;server.stop().await;
            }
        }
    }).await;
}

#[tokio::test]
async fn vertex_express_preview_three_protocols_complete_calls_roundtrip_and_settle() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::sequence(vec![preview_body(), success_body()]).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let mut request = public_wire::request_body(endpoint, stream);
                    opt_in(&mut request, json!(true));
                    let response = fixture.open(endpoint, request).await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let text = drain(response.into_body()).await;
                    let tools = public_wire::calls(&text, endpoint, stream);
                    assert_eq!(tools.len(), 2, "{text}");
                    assert_ne!(tools[0].id, tools[1].id);
                    for tool in &tools {
                        assert_eq!(tool.args, json!({"city":"杭州","nested":[1,2]}));
                    }
                    assert_eq!(fixture.audit().await.status, "completed");
                    assert_eq!(fixture.remaining().await, 1);
                    let response = fixture
                        .open(endpoint, public_wire::followup(endpoint, stream, &tools))
                        .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    assert!(drain(response.into_body()).await.contains("合成答案"));
                    assert_eq!(super::tools::audits(&fixture, 2).await.len(), 2);
                    assert_eq!(fixture.remaining().await, 0);
                    super::tools::assert_roundtrip_wire(&server, &tools);
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
async fn vertex_express_preview_invalid_option_rejected_before_network_or_debit() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                for value in [json!("true"), json!(1), json!(null)] {
                    let server = wire::Server::start(200, success_body()).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin, async {
                        let mut request = public_wire::request_body(endpoint, stream);
                        opt_in(&mut request, value);
                        let response = fixture.open(endpoint, request).await;
                        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
                        assert!(drain(response.into_body())
                            .await
                            .contains("vertex_express_invalid_tool_stream_option"));
                        assert_eq!(fixture.audit().await.status, "failed");
                        assert_eq!(fixture.remaining().await, 2);
                        assert!(server.calls.lock().unwrap().is_empty());
                    })
                    .await;
                    fixture.finish().await;
                    server.stop().await;
                }
            }
        }
    })
    .await;
}
