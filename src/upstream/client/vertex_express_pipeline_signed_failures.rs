use super::*;
#[tokio::test]
async fn vertex_express_signed_raw_state_cannot_bypass_typed_history() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
                for part in [json!({"text":"old","thoughtSignature":"skip_thought_signature_validator"}),json!({"text":"old","thought":true}),json!({"text":"old","thought_signature":"YWJj"})] {
                    let server=wire::Server::start(200,success_body()).await;
                    let fixture=Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin,async {
                        let mut req=public_wire::request_body(endpoint,stream);
                        let items=json!([{"role":"assistant","content":part},{"role":"user","content":"new"}]);
                        req[if endpoint=="responses" {"input"}else{"messages"}]=items;
                        let response=fixture.open(endpoint,req).await;
                        assert_eq!(response.status(),StatusCode::BAD_REQUEST);
                        assert!(drain(response.into_body()).await.contains("vertex_express_invalid_signed_history"));
                        assert_eq!(fixture.audit().await.status,"failed");assert_eq!(fixture.remaining().await,2);
                        assert!(server.calls.lock().unwrap().is_empty());
                    }).await;
                    fixture.finish().await;server.stop().await;
                }
            }
        }
    }).await;
}

fn material(calls: &[public_wire::Call]) -> Value {
    let mut index = 0;
    let parts: Vec<_> = parts()
        .as_array()
        .unwrap()
        .iter()
        .map(|part| {
            let mut value = json!({});
            for field in ["text", "thought"] {
                if let Some(v) = part.get(field) {
                    value[field] = v.clone();
                }
            }
            if let Some(signature) = part.get("thoughtSignature") {
                value["thought_signature"] = signature.clone();
            }
            if let Some(call) = part.get("functionCall") {
                value["function_call"] = call.clone();
                value["public_id"] = json!(calls[index].id);
                index += 1;
            }
            value
        })
        .collect();
    json!({"source":"vertex_express_api","model":"gemini-contract","parts":parts})
}
#[tokio::test]
async fn vertex_express_signed_history_mismatch_rejected_before_network_or_success_debit() {
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
                // Responses' closed extra-field whitelist ignores native contents instead of
                // forwarding it. Its non-override contract is covered separately below.
                for case in 0..if endpoint == "responses" { 12 } else { 13 } {
                    let server = wire::Server::start(200, success_body()).await;
                    let fixture = Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin, async {
                        let mut req = followup(endpoint, stream, &calls, material(&calls));
                        let history = &mut req["gateway_vertex_history"];
                        match case {
                            0 => history[0]["turn"]["source"] = json!("aistudio"),
                            1 => history[0]["turn"]["model"] = json!(MODEL),
                            2 => history[0]["turn"]["parts"][2]["public_id"] = json!("unknown"),
                            3 => {
                                history[0]["turn"]["parts"][2]["function_call"]["args"] =
                                    json!({"city":"wrong"})
                            }
                            4 => history[0]["turn"]["parts"][1]["text"] = json!("changed"),
                            5 => history[0]["assistant_turn"] = json!(1),
                            6 => {
                                let duplicate = history[0].clone();
                                history.as_array_mut().unwrap().push(duplicate);
                            }
                            7 => *history = Value::Null,
                            8 => history[0]["unknown"] = json!(true),
                            9 => {
                                history[0]["turn"]["parts"][0]["thought_signature"] =
                                    json!("private-invalid!")
                            }
                            10 => history[0]["turn"]["parts"]
                                .as_array_mut()
                                .unwrap()
                                .swap(2, 4),
                            11 => {
                                history[0]["turn"]["parts"][2]["function_call"]["name"] =
                                    json!("different")
                            }
                            _ => {}
                        }
                        if case == 12 {
                            req["contents"] = json!([]);
                        }
                        let response = fixture.open(endpoint, req).await;
                        assert_eq!(
                            response.status(),
                            StatusCode::BAD_REQUEST,
                            "{endpoint}/{stream}/{case}"
                        );
                        let text = drain(response.into_body()).await;
                        assert!(
                            text.contains("vertex_express_invalid_signed_history"),
                            "{text}"
                        );
                        assert!(!text.contains("private-invalid") && !text.contains(KEY));
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
async fn vertex_express_signed_responses_ignored_contents_cannot_replace_typed_history() {
    Box::pin(async {
        for stream in [false, true] {
            let server = wire::Server::sequence(vec![signed_body(), success_body()]).await;
            let fixture = Box::pin(Fixture::new(&server)).await;
            wire::within(&server.origin, async {
                let response = fixture
                    .open("responses", public_wire::request_body("responses", stream))
                    .await;
                assert_eq!(response.status(), StatusCode::OK);
                let text = drain(response.into_body()).await;
                let calls = public_wire::calls(&text, "responses", stream);
                let mut req = followup(
                    "responses",
                    stream,
                    &calls,
                    turn(&text, "responses", stream),
                );
                req["contents"] = json!([{"role":"model","parts":[{"text":"wrong-override"}]}]);
                let response = fixture.open("responses", req).await;
                assert_eq!(response.status(), StatusCode::OK);
                assert!(drain(response.into_body()).await.contains("合成答案"));
                assert_eq!(fixture.remaining().await, 0);
                assert_eq!(super::super::tools::audits(&fixture, 2).await.len(), 2);
                let captured = server.calls.lock().unwrap();
                assert_eq!(captured.len(), 2);
                assert_eq!(captured[1].2["contents"][1]["parts"], parts());
                assert!(!captured[1].2.to_string().contains("wrong-override"));
                assert!(captured[1].2.get("gateway_vertex_history").is_none());
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn vertex_express_signed_invalid_upstream_never_becomes_success_or_replays() {
    Box::pin(async {
        for part in [json!({"text":"private","thoughtSignature":"invalid!"}),
            json!({"functionCall":{"name":"lookup","partialArgs":[{"jsonPath":"$.city","stringValue":"杭州"}]},"thoughtSignature":"YWJj"})] {
            for endpoint in ["chat/completions","messages","responses"] {
                for stream in [false,true] {
                    let server=wire::Server::start(200,event(json!({"candidates":[{"content":{"parts":[part.clone()]},"finishReason":"STOP"}],"usageMetadata":{"totalTokenCount":5}}))).await;
                    let fixture=Box::pin(Fixture::new(&server)).await;
                    wire::within(&server.origin,async {
                        let response=fixture.open(endpoint,public_wire::request_body(endpoint,stream)).await;
                        assert_eq!(response.status(),StatusCode::BAD_GATEWAY);
                        let text=drain(response.into_body()).await;
                        for private in ["private","invalid!","YWJj",KEY,"[DONE]","response.completed","message_stop"]{assert!(!text.contains(private),"{text}");}
                        assert_eq!(fixture.audit().await.status,"failed");assert_eq!(fixture.remaining().await,2);
                        assert_eq!(server.calls.lock().unwrap().len(),1);
                    }).await;
                    fixture.finish().await;server.stop().await;
                }
            }
        }
    }).await;
}
#[tokio::test]
async fn vertex_express_signed_filtered_turn_never_exposes_executable_calls_or_state() {
    Box::pin(async {
        for endpoint in ["chat/completions","messages","responses"] {
            for stream in [false,true] {
                let server=wire::Server::start(200,event(json!({"candidates":[{"content":{"parts":parts()},"finishReason":"SAFETY"}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5}}))).await;
                let fixture=Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin,async {
                    let response=fixture.open(endpoint,public_wire::request_body(endpoint,stream)).await;
                    assert_eq!(response.status(),StatusCode::OK);
                    let text=drain(response.into_body()).await;
                    for hidden in ["summary-only","YWJj","ZGVm","gateway_vertex_turn","lookup"]{assert!(!text.contains(hidden),"{text}");}
                    let row=fixture.audit().await;assert_eq!(row.status,"completed");assert_eq!(row.total_tokens,Some(5));
                    assert_eq!(fixture.remaining().await,1);assert_eq!(server.calls.lock().unwrap().len(),1);
                }).await;
                fixture.finish().await;server.stop().await;
            }
        }
    }).await;
}
