//! HTTP 200 不代表推理成功；含第二可调用候选时也不重放已收到的无效生成。
use super::*;

#[tokio::test]
async fn commandcode_native_json_preserves_xml_text_and_tool_argument_entities() {
    Box::pin(async {
        let xml="<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>";
        let args=json!({"text":"&lt;&amp;&quot;"});
        for endpoint in ENDPOINTS {
            for tools in [false,true] {
                let message=if tools { json!({"role":"assistant","content":null,"tool_calls":[{
                    "id":"native-call","type":"function","function":{"name":"lookup","arguments":args.to_string()}}]})
                } else { json!({"role":"assistant","content":xml}) };
                let raw=json!({"id":"chatcmpl-native","model":UPSTREAM_MODEL,"choices":[{"index":0,"message":message,
                    "finish_reason":if tools {"tool_calls"} else {"stop"}}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}});
                let peer=peer::Peer::raw(raw.to_string().into_bytes()).await;
                let fixture=Box::pin(Fixture::new(&peer)).await;
                super::super::test_support::origin(peer.origin.clone(),async {
                    let response=fixture.open(endpoint,body(endpoint,false)).await;
                    assert_eq!(response.status(),StatusCode::OK);
                    let text=drain(response.into_body()).await;
                    if tools {
                        let calls=public_wire::calls(&text,endpoint,false);
                        assert_eq!(calls.len(),1,"{text}"); assert_eq!(calls[0].args,args);
                        assert_eq!(calls[0].id,"native-call");
                    } else {
                        let value: Value=serde_json::from_str(&text).unwrap();
                        match endpoint {
                            "messages" => {
                                assert_eq!(value["content"][0]["text"],xml);
                                assert!(value["content"].as_array().unwrap().iter().all(|v| v["type"] != "tool_use"));
                            }
                            "responses" => {
                                assert_eq!(value["output"][0]["content"][0]["text"],xml);
                                assert!(value["output"].as_array().unwrap().iter().all(|v| v["type"] != "function_call"));
                            }
                            _ => {
                                assert_eq!(value["choices"][0]["message"]["content"],xml);
                                assert!(value["choices"][0]["message"].get("tool_calls").is_none());
                                assert_eq!(value["choices"][0]["finish_reason"],"stop");
                            }
                        }
                    }
                    assert_eq!(fixture.audits(1).await[0].status,"completed");
                    assert_eq!(fixture.remaining().await,1); assert_wire(&peer,1,endpoint);
                }).await;
                fixture.finish().await; peer.stop().await;
            }
        }
    }).await;
}
#[tokio::test]
async fn commandcode_invalid_success_body_refunds_without_replay() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for raw in [json!({"error":{"code":16,"message":KEY}}), json!({"choices":[]}),
                json!({"choices":[{"message":{"role":"assistant","content":"unfinished"},"finish_reason":null}]}),
                json!({"choices":[{"message":{"role":"assistant","content":null},"finish_reason":"tool_calls"}]})] {
                let peer = peer::Peer::raw(raw.to_string().into_bytes()).await;
                let fixture = Box::pin(Fixture::with_fallback(&peer,true)).await;
                super::super::test_support::origin(peer.origin.clone(),async {
                    let response = fixture.open(endpoint,body(endpoint,false)).await;
                    assert_eq!(response.status(),StatusCode::BAD_GATEWAY);
                    let text = drain(response.into_body()).await;
                    assert!(text.contains("commandcode_response_failed") && !text.contains(KEY),"{text}");
                    assert_ne!(fixture.audits(1).await[0].status,"completed");
                    assert_eq!(fixture.remaining().await,2); assert_wire(&peer,1,endpoint);
                }).await;
                fixture.finish().await; peer.stop().await;
            }
        }
    }).await;
}
