//! Explicit Full Vertex Preview opt-in uses the same bounded assembler, not another transport.
use super::{
    tools::{self, public_wire},
    *,
};
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
    body.extend(event(
        json!({"candidates":[{"finishReason":"STOP"}],"usageMetadata":{"totalTokenCount":5}}),
    ));
    body
}
fn option(req: &mut Value, enabled: Value) {
    req["toolConfig"] = json!({"functionCallingConfig":{"streamFunctionCallArguments":enabled}});
}
#[tokio::test]
async fn vertex_oauth_preview_three_protocols_opt_in_and_complete_calls_roundtrip() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
                for enabled in [false,true] {
                    let server=wire::Server::sequence(vec![if enabled {preview_body()}else{success_body()},success_body()]).await;
                    let fixture=Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin,async {
                        let mut req=public_wire::request_body(endpoint,stream);option(&mut req,json!(enabled));
                        let response=fixture.open(endpoint,req).await;let status=response.status();let text=drain(response.into_body()).await;
                        assert_eq!(status,StatusCode::OK,"{text}");assert_eq!(fixture.audit().await.status,"completed");assert_eq!(fixture.remaining().await,1);
                        assert_eq!(server.calls.lock().unwrap()[0].2["toolConfig"]["functionCallingConfig"],json!({"mode":"ANY","allowedFunctionNames":["lookup"],"streamFunctionCallArguments":enabled}));
                        if enabled {
                            let calls=public_wire::calls(&text,endpoint,stream);assert_eq!(calls.len(),2);assert_ne!(calls[0].id,calls[1].id);
                            for call in &calls {assert_eq!(call.args,json!({"city":"杭州","nested":[1,2]}));}
                            let response=fixture.open(endpoint,public_wire::followup(endpoint,stream,&calls)).await;
                            assert_eq!(response.status(),StatusCode::OK);assert!(drain(response.into_body()).await.contains("合成答案"));
                            assert_eq!(tools::audits(&fixture,2).await.len(),2);assert_eq!(fixture.remaining().await,0);tools::assert_roundtrip(&server,&calls);
                        } else {assert!(text.contains("合成答案"));assert_wire(&server,1);}
                    }).await;
                    fixture.finish().await;server.stop().await;
                }
            }
        }
    }).await;
}
#[tokio::test]
async fn vertex_oauth_preview_invalid_option_rejected_before_network_and_success_debit() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                for enabled in [json!("true"), json!(1), json!(null)] {
                    let server = wire::Server::start(200, success_body()).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin, async {
                        let mut req = public_wire::request_body(endpoint, stream);
                        option(&mut req, enabled);
                        let response = fixture.open(endpoint, req).await;
                        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
                        assert!(drain(response.into_body())
                            .await
                            .contains("vertex_oauth_invalid_tool_stream_option"));
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
#[tokio::test]
async fn vertex_oauth_preview_caller_drop_closes_partial_source_and_refunds() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
                let raw=event(json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup","willContinue":true}}]}}]}));
                let server=wire::Server::stream(200,raw,true).await;let fixture=Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin,async {
                    let mut req=public_wire::request_body(endpoint,stream);option(&mut req,json!(true));
                    let mut call=Box::pin(fixture.open(endpoint,req));assert!(tokio::time::timeout(Duration::from_millis(200),call.as_mut()).await.is_err());
                    assert_wire(&server,1);assert_eq!(fixture.remaining().await,1);drop(call);
                    assert_eq!(fixture.audit().await.status,"cancelled");assert_eq!(fixture.remaining().await,2);
                    tokio::time::timeout(Duration::from_secs(3),async {while server.closed()==0 {tokio::task::yield_now().await;}}).await.unwrap();
                    assert_eq!(server.calls.lock().unwrap().len(),1);
                }).await;fixture.finish().await;server.stop().await;
            }
        }
    }).await;
}
