//! Native upstream envelopes, never pretranslated public replies; tool IDs survive a second turn.
use super::*;
pub(super) fn args() -> Value {
    json!({"city":"杭州","nested":[1,2]})
}
fn tool(index: usize) -> Value {
    json!({"id":format!("call-{index}"),"type":"function","function":{"name":"lookup","arguments":args().to_string()}})
}
fn usage() -> Value {
    json!({"input_tokens":2,"output_tokens":3,"total_tokens":5})
}
fn frame(value: Value) -> String {
    format!("data: {value}\n\n")
}
fn message_frame(value: Value) -> String {
    // Native Messages names its events; the usage tap treats message_start as provisional.
    let event = value["type"].as_str().expect("native Messages event type");
    format!("event: {event}\ndata: {value}\n\n")
}
pub(super) fn reply(family: &str, tools: bool, stream: bool, terminal: bool) -> Vec<u8> {
    match family {
        "chat" => chat(tools, stream, terminal),
        "responses" => responses(tools, stream, terminal),
        "messages" => messages(tools, stream, terminal),
        _ => gemini(tools, stream, terminal),
    }
}
fn chat(tools: bool, stream: bool, terminal: bool) -> Vec<u8> {
    let message = if tools {
        json!({"role":"assistant","content":null,"tool_calls":[tool(0),tool(1)]})
    } else {
        json!({"role":"assistant","content":"合成答案"})
    };
    let reason = if tools { "tool_calls" } else { "stop" };
    let usage = json!({"prompt_tokens":2,"completion_tokens":3,"total_tokens":5});
    if !stream {
        return json!({"id":"native-chat","object":"chat.completion","model":"opaque-model","choices":[{"index":0,"message":message,"finish_reason":reason}],"usage":usage}).to_string().into_bytes();
    }
    let delta = if tools {
        let mut items = vec![tool(0), tool(1)];
        for (i, item) in items.iter_mut().enumerate() {
            item["index"] = json!(i);
        }
        json!({"role":"assistant","tool_calls":items})
    } else {
        json!({"role":"assistant","content":"合成答案"})
    };
    let mut out = frame(
        json!({"id":"native-chat","object":"chat.completion.chunk","model":"opaque-model","choices":[{"index":0,"delta":delta,"finish_reason":null}]}),
    );
    if terminal {
        out += &frame(
            json!({"id":"native-chat","choices":[{"index":0,"delta":{},"finish_reason":reason}],"usage":usage}),
        );
        out += "data: [DONE]\n\n";
    }
    out.into_bytes()
}
fn response_items(tools: bool) -> Vec<Value> {
    if tools {
        (0..2).map(|i|json!({"type":"function_call","id":format!("item-{i}"),"call_id":format!("call-{i}"),"name":"lookup","arguments":args().to_string(),"status":"completed"})).collect()
    } else {
        vec![
            json!({"type":"message","id":"native-msg","role":"assistant","status":"completed","content":[{"type":"output_text","text":"合成答案","annotations":[]}]}),
        ]
    }
}
fn responses(tools: bool, stream: bool, terminal: bool) -> Vec<u8> {
    let items = response_items(tools);
    let response = json!({"id":"native-resp","object":"response","model":"opaque-model","status":"completed","output":items,"usage":usage()});
    if !stream {
        return response.to_string().into_bytes();
    }
    let mut out = frame(
        json!({"type":"response.created","response":{"id":"native-resp","object":"response","status":"in_progress","output":[]}}),
    );
    for (i, item) in items.iter().enumerate() {
        let mut added = item.clone();
        added["status"] = json!("in_progress");
        if tools {
            added["arguments"] = json!("");
        } else {
            added["content"] = json!([]);
        }
        out += &frame(json!({"type":"response.output_item.added","output_index":i,"item":added}));
        if tools {
            out += &frame(
                json!({"type":"response.function_call_arguments.delta","output_index":i,"item_id":item["id"],"delta":args().to_string()}),
            );
            if terminal {
                out += &frame(
                    json!({"type":"response.function_call_arguments.done","output_index":i,"item_id":item["id"],"arguments":args().to_string()}),
                );
            }
        } else {
            out += &frame(
                json!({"type":"response.content_part.added","output_index":i,"item_id":item["id"],"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}),
            );
            out += &frame(
                json!({"type":"response.output_text.delta","output_index":i,"item_id":item["id"],"content_index":0,"delta":"合成答案"}),
            );
            if terminal {
                out += &frame(
                    json!({"type":"response.output_text.done","output_index":i,"item_id":item["id"],"content_index":0,"text":"合成答案"}),
                );
                out += &frame(
                    json!({"type":"response.content_part.done","output_index":i,"item_id":item["id"],"content_index":0,"part":item["content"][0]}),
                );
            }
        }
        if terminal {
            out += &frame(json!({"type":"response.output_item.done","output_index":i,"item":item}));
        }
    }
    if terminal {
        out += &frame(json!({"type":"response.completed","response":response}));
    }
    out.into_bytes()
}
fn messages(tools: bool, stream: bool, terminal: bool) -> Vec<u8> {
    let content: Vec<_> = if tools {
        (0..2).map(|i|json!({"type":"tool_use","id":format!("call-{i}"),"name":"lookup","input":args()})).collect()
    } else {
        vec![json!({"type":"text","text":"合成答案"})]
    };
    let reason = if tools { "tool_use" } else { "end_turn" };
    let result = json!({"id":"native-msg","type":"message","role":"assistant","model":"opaque-model","content":content,"stop_reason":reason,"usage":{"input_tokens":2,"output_tokens":3}});
    if !stream {
        return result.to_string().into_bytes();
    }
    let mut out = message_frame(
        json!({"type":"message_start","message":{"id":"native-msg","type":"message","role":"assistant","model":"opaque-model","content":[],"stop_reason":null,"usage":{"input_tokens":2,"output_tokens":0}}}),
    );
    for (i, item) in content.iter().enumerate() {
        let mut start = item.clone();
        if tools {
            start["input"] = json!({});
        } else {
            start["text"] = json!("");
        }
        out +=
            &message_frame(json!({"type":"content_block_start","index":i,"content_block":start}));
        out += &message_frame(
            json!({"type":"content_block_delta","index":i,"delta":if tools{json!({"type":"input_json_delta","partial_json":args().to_string()})}else{json!({"type":"text_delta","text":"合成答案"})}}),
        );
        if terminal {
            out += &message_frame(json!({"type":"content_block_stop","index":i}));
        }
    }
    if terminal {
        out += &message_frame(
            json!({"type":"message_delta","delta":{"stop_reason":reason},"usage":{"output_tokens":3}}),
        );
        out += &message_frame(json!({"type":"message_stop"}));
    }
    out.into_bytes()
}
fn gemini(tools: bool, stream: bool, terminal: bool) -> Vec<u8> {
    let parts = if tools {
        json!([{"functionCall":{"name":"lookup","args":args()}},{"functionCall":{"name":"lookup","args":args()}}])
    } else {
        json!([{"text":"合成答案"}])
    };
    let mut result = json!({"candidates":[{"index":0,"content":{"role":"model","parts":parts}}]});
    if terminal {
        result["candidates"][0]["finishReason"] = json!("STOP");
        result["usageMetadata"] =
            json!({"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5});
    }
    if stream {
        frame(result).into_bytes()
    } else {
        result.to_string().into_bytes()
    }
}
pub(super) fn assert_tools(captured: &peer::Calls, family: &str, calls: &[public_wire::Call]) {
    let first = &captured[0].2;
    let second = &captured[1].2;
    let schema = match family {
        "chat" => &first["tools"][0]["function"]["parameters"],
        "responses" => &first["tools"][0]["parameters"],
        "messages" => &first["tools"][0]["input_schema"],
        _ => &first["tools"][0]["functionDeclarations"][0]["parameters"],
    };
    assert_eq!(schema, &public_wire::schema(), "{family}: {first}");
    if family == "gemini" {
        let contents = second["contents"].as_array().unwrap();
        let functions: Vec<_> = contents
            .iter()
            .flat_map(|v| v["parts"].as_array().unwrap())
            .filter_map(|v| v.get("functionCall"))
            .collect();
        let groups: Vec<_> = contents
            .iter()
            .filter(|v| {
                v["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p.get("functionResponse").is_some())
            })
            .collect();
        assert_eq!(functions.len(), calls.len());
        assert_eq!(groups.len(), 1, "{second}");
        let results = groups[0]["parts"].as_array().unwrap();
        assert_eq!(results.len(), calls.len());
        for (i, call) in calls.iter().enumerate() {
            assert_eq!(functions[i]["name"], call.name);
            assert_eq!(functions[i]["args"], call.args);
            assert_eq!(results[i]["functionResponse"]["name"], call.name);
            assert_eq!(
                results[i]["functionResponse"]["response"],
                json!({"ordinal":i,"city":"杭州"})
            );
        }
        return;
    }
    let results: Vec<(&str, Value)> = match family {
        "chat" => second["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["role"] == "tool")
            .map(|v| {
                (
                    v["tool_call_id"].as_str().unwrap(),
                    serde_json::from_str(v["content"].as_str().unwrap()).unwrap(),
                )
            })
            .collect(),
        "responses" => second["input"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "function_call_output")
            .map(|v| {
                (
                    v["call_id"].as_str().unwrap(),
                    serde_json::from_str(v["output"].as_str().unwrap()).unwrap(),
                )
            })
            .collect(),
        "messages" => second["messages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|v| v["content"].as_array().into_iter().flatten())
            .filter(|v| v["type"] == "tool_result")
            .map(|v| {
                (
                    v["tool_use_id"].as_str().unwrap(),
                    serde_json::from_str(v["content"].as_str().unwrap()).unwrap(),
                )
            })
            .collect(),
        _ => unreachable!(),
    };
    assert_eq!(results.len(), 2, "{family}: {second}");
    for (ordinal, call) in calls.iter().enumerate() {
        let matched: Vec<_> = results.iter().filter(|(id, _)| *id == call.id).collect();
        assert_eq!(matched.len(), 1, "{family}: {results:?}");
        assert_eq!(matched[0].1, json!({"ordinal":ordinal,"city":"杭州"}));
    }
}
