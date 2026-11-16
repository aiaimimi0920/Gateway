use super::*;

#[tokio::test]
async fn vertex_oauth_signed_mismatched_or_cross_product_history_never_sends_or_debits_success() {
    Box::pin(async {
        let calls: Vec<_> = (0..2)
            .map(|i| public_wire::Call {
                id: format!("c{i}"),
                name: "lookup".into(),
                args: if i == 0 {
                    json!({"city":"杭州","nested":[1,2],"literal":"&amp; &quot;"})
                } else {
                    json!({"city":"杭州","nested":[1,2]})
                },
            })
            .collect();
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                for case in 0..if endpoint == "responses" { 6 } else { 7 } {
                    let server = wire::Server::start(200, success_body()).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    let mut req = followup(endpoint, stream, &calls, material(&calls));
                    match case {
                        0 => {
                            req["gateway_vertex_history"][0]["turn"]["source"] =
                                json!("vertex_express_api")
                        }
                        1 => req["gateway_vertex_history"][0]["turn"]["model"] = json!(MODEL),
                        2 => {
                            req["gateway_vertex_history"][0]["turn"]["parts"][2]["public_id"] =
                                json!("wrong")
                        }
                        3 => {
                            req["gateway_vertex_history"][0]["turn"]["parts"][2]["function_call"]
                                ["args"] = json!({"city":"wrong"})
                        }
                        4 => {
                            req["gateway_vertex_history"][0]["turn"]["parts"][1]["text"] =
                                json!("changed")
                        }
                        5 => req["gateway_vertex_history"][0]["assistant_turn"] = json!(1),
                        _ => req["contents"] = json!([]),
                    }
                    wire::within(&server.origin, async {
                        let response = fixture.open(endpoint, req).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(
                            status,
                            StatusCode::BAD_REQUEST,
                            "{endpoint}/{stream}/{case}: {text}"
                        );
                        assert!(
                            text.contains("vertex_oauth_invalid_signed_history")
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
        }
    })
    .await;
}

#[tokio::test]
async fn vertex_oauth_signed_raw_state_cannot_bypass_typed_history() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
                for part in [json!({"text":"old","thoughtSignature":"skip_thought_signature_validator"}),json!({"text":"old","thought":true}),json!({"text":"old","thought_signature":"YWJj"})] {
                    let server=wire::Server::start(200,success_body()).await;
                    let fixture=Box::pin(Fixture::new(&server)).await;
                    let mut req=public_wire::request_body(endpoint,stream);
                    req[if endpoint=="responses" {"input"}else{"messages"}]=json!([{"role":"assistant","content":part},{"role":"user","content":"new"}]);
                    wire::within(&server.origin,async {
                        let response=fixture.open(endpoint,req).await;let status=response.status();let text=drain(response.into_body()).await;
                        assert_eq!(status,StatusCode::BAD_REQUEST,"{text}");assert!(text.contains("vertex_oauth_invalid_signed_history"));
                        assert_eq!(fixture.audit().await.status,"failed");assert_eq!(fixture.remaining().await,2);assert!(server.calls.lock().unwrap().is_empty());
                    }).await;
                    fixture.finish().await;server.stop().await;
                }
            }
        }
    }).await;
}

#[tokio::test]
async fn vertex_oauth_signed_invalid_upstream_state_or_tools_abort_without_replay() {
    Box::pin(async {
        for part in [json!({"text":"private-state","thoughtSignature":"invalid!"}),
            json!({"functionCall":{"name":"lookup","partialArgs":[{"jsonPath":"$.city","stringValue":"杭州"}]},"thoughtSignature":"YWJj"}),
            json!({"functionCall":{"name":"lookup","args":"private-state"}}),
            json!({"inlineData":{"mimeType":"image/png","data":"private-state"}})] {
            for endpoint in ["chat/completions","messages","responses"] {
                for stream in [false,true] {
                    let server=wire::Server::start(200,event(json!({"candidates":[{"content":{"parts":[part.clone()]},"finishReason":"STOP"}],"usageMetadata":{"totalTokenCount":5}}))).await;
                    let fixture=Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin,async {
                        let response=fixture.open(endpoint,public_wire::request_body(endpoint,stream)).await;
                        assert_eq!(response.status(),StatusCode::BAD_GATEWAY);let text=drain(response.into_body()).await;
                        for private in ["private-state","invalid!","YWJj",ACCESS,"[DONE]","response.completed","message_stop"] {assert!(!text.contains(private),"{text}");}
                        assert_eq!(fixture.audit().await.status,"failed");assert_eq!(fixture.remaining().await,2);assert_wire(&server,1);
                    }).await;
                    fixture.finish().await;server.stop().await;
                }
            }
        }
    }).await;
}

#[tokio::test]
async fn vertex_oauth_signed_filter_discards_tools_and_state_but_preserves_reported_usage() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
                let server=wire::Server::start(200,event(json!({"candidates":[{"content":{"parts":parts()},"finishReason":"SAFETY"}],"usageMetadata":{"totalTokenCount":5}}))).await;
                let fixture=Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin,async {
                    let response=fixture.open(endpoint,public_wire::request_body(endpoint,stream)).await;
                    assert_eq!(response.status(),StatusCode::OK);let text=drain(response.into_body()).await;
                    for private in ["summary-only","lookup","YWJj","ZGVm","gateway_vertex_turn",ACCESS] {assert!(!text.contains(private),"{text}");}
                    let row=fixture.audit().await;assert_eq!(row.status,"completed");assert_eq!(row.total_tokens,Some(5));assert_identity(&row);
                    assert_eq!(fixture.remaining().await,1);assert_wire(&server,1);
                }).await;
                fixture.finish().await;server.stop().await;
            }
        }
    }).await;
}
