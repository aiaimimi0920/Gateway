//! Reject incomplete, mismatched and unbounded public continuation envelopes before sending.
use super::*;
use neuro_gateway::{routing::candidate::ProviderAccountPayload, upstream::client::UpstreamClient};

fn rejected(protocol: u8, body: Value) {
    let req = normalize(protocol, body);
    let error = bedrock_converse::try_pack_bedrock_converse(&req, "fixture", false).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("bedrock_invalid_signed_history")
    );
}
#[test]
fn continuation_scope_is_closed_and_must_match_the_entire_prefix() {
    for protocol in 0..3 {
        for mode in 0..8 {
            let mut body = followup(protocol, turn(protocol));
            let scope = &mut body["gateway_bedrock_history"][0]["turn"]["continuation"];
            match mode {
                0 => scope["source"] = json!("other"),
                1 => scope["model"] = json!("other"),
                2 => scope["request_sha256"] = json!("0".repeat(64)),
                3 => scope["request_sha256"] = json!("A".repeat(64)),
                4 => scope["extra"] = json!(true),
                5 => *scope = Value::Null,
                6 => {
                    let key = if protocol == 2 { "input" } else { "messages" };
                    body[key][0]["content"] = json!("changed");
                }
                _ => {
                    let key = if protocol == 2 {
                        "instructions"
                    } else {
                        "system"
                    };
                    body[key] = json!("changed system");
                    if protocol == 0 {
                        body["messages"]
                            .as_array_mut()
                            .unwrap()
                            .insert(0, json!({"role":"system","content":"changed system"}));
                    }
                }
            }
            rejected(protocol, body);
        }
    }
}
#[test]
fn indexed_history_schema_rejects_duplicate_unsorted_missing_and_out_of_range_turns() {
    for protocol in 0..3 {
        for mode in 0..9 {
            let mut body = followup(protocol, turn(protocol));
            let entry = body["gateway_bedrock_history"][0].clone();
            match mode {
                0 => body["gateway_bedrock_history"] = json!([entry.clone(), entry]),
                1 => body["gateway_bedrock_history"][0]["assistant_turn"] = json!(1),
                2 => body["gateway_bedrock_history"][0]["assistant_turn"] = json!(-1),
                3 => body["gateway_bedrock_history"][0]["assistant_turn"] = json!(0.5),
                4 => body["gateway_bedrock_history"][0]["unknown"] = json!(true),
                5 => {
                    body["gateway_bedrock_history"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("turn");
                }
                6 => body["gateway_bedrock_history"] = json!([]),
                7 => body["gateway_bedrock_history"] = json!({}),
                _ => body["gateway_bedrock_history"] = json!(vec![entry; 65]),
            }
            rejected(protocol, body);
        }
    }
}
#[test]
fn visible_text_tool_identity_and_literal_arguments_cannot_disagree_with_the_turn() {
    for protocol in 0..3 {
        for mode in 0..4 {
            let mut state = turn(protocol);
            match mode {
                0 => state["content"][0]["text"] = json!("changed"),
                1 => state["content"][2]["toolUse"]["toolUseId"] = json!("other"),
                2 => state["content"][2]["toolUse"]["name"] = json!("other"),
                _ => state["content"][2]["toolUse"]["input"] = json!({"v":"\""}),
            }
            rejected(protocol, followup(protocol, state));
        }
    }
}
#[test]
fn history_union_base64_and_turn_budget_are_validated() {
    for protocol in 0..3 {
        for mode in 0..4 {
            let mut state = turn(protocol);
            match mode {
                0 => {
                    state["content"][1]["reasoningContent"]["reasoningText"]["signature"] =
                        Value::Null
                }
                1 => state["content"][1]["text"] = json!("competing"),
                2 => {
                    state["content"][3]["reasoningContent"]["redactedContent"] = json!("not base64")
                }
                _ => {
                    state["content"][1]["reasoningContent"]["reasoningText"]["text"] =
                        json!("x".repeat(4 * 1024 * 1024))
                }
            }
            rejected(protocol, followup(protocol, state));
        }
    }
    let mut body = followup(0, turn(0));
    body["gateway_bedrock_history"][0]["turn"]["unknown"] = json!("x".repeat(8 * 1024 * 1024));
    rejected(0, body);
}
#[test]
fn incomplete_duplicate_and_foreign_tool_results_are_not_silently_rewritten() {
    for protocol in 0..3 {
        for mode in 0..3 {
            let mut body = followup(protocol, turn(protocol));
            let key = if protocol == 2 { "input" } else { "messages" };
            let array = body[key].as_array_mut().unwrap();
            match mode {
                0 => {
                    array.pop();
                }
                1 => {
                    array.push(array.last().unwrap().clone());
                }
                _ => {
                    let result = array.last_mut().unwrap();
                    match protocol {
                        0 => result["tool_call_id"] = json!("foreign"),
                        1 => result["content"][0]["tool_use_id"] = json!("foreign"),
                        _ => result["call_id"] = json!("foreign"),
                    }
                }
            }
            rejected(protocol, body);
        }
    }
}
#[test]
fn malformed_tool_arguments_cannot_match_a_fabricated_empty_object() {
    for protocol in [0, 2] {
        let mut state = turn(protocol);
        state["content"][2]["toolUse"]["input"] = json!({});
        let mut body = followup(protocol, state);
        if protocol == 0 {
            body["messages"][1]["tool_calls"][0]["function"]["arguments"] = json!("{");
        } else {
            body["input"][2]["arguments"] = json!("{");
        }
        rejected(protocol, body);
    }
}
#[test]
fn bedrock_and_vertex_history_cannot_be_combined() {
    let mut body = followup(0, turn(0));
    body["gateway_vertex_history"] = json!([]);
    rejected(0, body);
}
#[test]
fn history_and_response_carriers_cannot_escape_into_other_product_plans() {
    for adapter in [
        "openai_compatible",
        "anthropic_compatible",
        "grok_compatible",
        "custom_http",
        "freebuff_compatible",
        "kiro_compatible",
    ] {
        let payload: ProviderAccountPayload = serde_json::from_value(
            json!({"adapter":adapter,"base_url":"http://127.0.0.1:9","api_key":"synthetic-only"}),
        )
        .unwrap();
        let req = normalize(0, followup(0, turn(0)));
        assert!(
            UpstreamClient::build_request_plan(&payload, &req, "fixture", false).is_err(),
            "{adapter}"
        );
        for field in [
            "gateway_bedrock_turn",
            "gateway_bedrock_event",
            "gateway_bedrock_terminal",
        ] {
            let mut body = first(0);
            body[field] = json!({});
            assert!(
                UpstreamClient::build_request_plan(&payload, &normalize(0, body), "fixture", false)
                    .is_err(),
                "{adapter}, {field}"
            );
        }
    }
}
#[test]
fn public_tool_results_and_responses_assistant_items_form_one_native_turn() {
    let mut body = followup(2, turn(2));
    body["gateway_bedrock_history"][0]["turn"]["content"]
        .as_array_mut()
        .unwrap()
        .insert(
            3,
            json!({"toolUse":{"toolUseId":"call-b","name":"lookup","input":{}}}),
        );
    let array = body["input"].as_array_mut().unwrap();
    array.insert(
        3,
        json!({"type":"function_call","call_id":"call-b","name":"lookup","arguments":"{}"}),
    );
    array.insert(
        4,
        json!({"type":"function_call_output","call_id":"call-b","output":"{\"ok\":true}"}),
    );
    let native =
        bedrock_converse::try_pack_bedrock_converse(&normalize(2, body), "fixture", false).unwrap();
    assert_eq!(native["messages"].as_array().unwrap().len(), 3);
    assert_eq!(
        native["messages"][2]["content"][0]["toolResult"],
        json!({"toolUseId":"call-b","content":[{"text":"{\"ok\":true}"}]})
    );
    assert_eq!(
        native["messages"][2]["content"][1]["toolResult"]["toolUseId"],
        "call-a"
    );
}

#[test]
fn resolved_model_paths_and_provider_default_carriers_cannot_override_history() {
    let req = normalize(0, followup(0, turn(0)));
    for mode in 0..5 {
        let mut provider:ProviderAccountPayload=serde_json::from_value(json!({"adapter":"bedrock_converse_compatible","base_url":"http://127.0.0.1:9","api_key":"synthetic-only"})).unwrap();
        match mode {
            0 => provider.chat_completions_path = Some("/model/other/converse".into()),
            1 => {
                provider.extra_body =
                    Some(serde_json::from_value(json!({"system":[{"text":"changed"}]})).unwrap())
            }
            2 => {
                provider.extra_body =
                    Some(serde_json::from_value(json!({"gateway_bedrock_history":[]})).unwrap())
            }
            3 => {
                provider.extra_body =
                    Some(serde_json::from_value(json!({"gateway_bedrock_turn":{}})).unwrap())
            }
            _ => {
                provider.extra_body =
                    Some(serde_json::from_value(json!({"gateway_bedrock_event":{}})).unwrap())
            }
        }
        assert!(UpstreamClient::build_request_plan(&provider, &req, "fixture", false).is_err());
    }
}
fn wrong_product<T>(result: Result<T, neuro_gateway::error::GatewayError>) {
    let error = result
        .err()
        .expect("must reject before any network request");
    assert_eq!(
        error.code.as_deref(),
        Some("bedrock_signed_history_wrong_product")
    );
}
#[test]
fn direct_product_shortcuts_reject_public_history_before_any_transport() {
    std::thread::Builder::new().stack_size(16*1024*1024).spawn(||{
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let client=UpstreamClient::new(1);
            for adapter in ["openai_compatible","anthropic_compatible","grok_compatible","custom_http","freebuff_compatible","kiro_compatible","qwen_web_compatible","lumalabs_compatible"] {
                let provider:ProviderAccountPayload=serde_json::from_value(json!({"adapter":adapter,"base_url":"http://127.0.0.1:9","api_key":"synthetic-only"})).unwrap();
                let req=normalize(0,followup(0,turn(0)));
                wrong_product(client.execute(&provider,&req,"fixture",None).await);
                wrong_product(client.execute_stream(&provider,&req,"fixture",None).await);
                wrong_product(client.execute_stream_with_provider_account_id("exact",&provider,&req,"fixture",None).await);
                wrong_product(client.execute_json_passthrough("exact",neuro_gateway::routing::candidate::ProviderExecutionMode::DirectHttp,&provider,&req,"fixture",None).await);
                wrong_product(client.execute_binary_passthrough(&provider,&req,"fixture",None).await);
            }
        });
    }).unwrap().join().unwrap();
}
