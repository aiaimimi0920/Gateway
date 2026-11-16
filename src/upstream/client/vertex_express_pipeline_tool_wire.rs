//! Read public tool envelopes and return those exact identities/results on the next request.
use super::*;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(in super::super) struct Call {
    pub id: String,
    pub name: String,
    pub args: Value,
}

fn from_item(item: &Value, endpoint: &str) -> Call {
    match endpoint {
        "messages" => Call {
            id: item["id"].as_str().unwrap().into(),
            name: item["name"].as_str().unwrap().into(),
            args: item["input"].clone(),
        },
        "responses" => Call {
            id: item["call_id"].as_str().unwrap().into(),
            name: item["name"].as_str().unwrap().into(),
            args: serde_json::from_str(item["arguments"].as_str().unwrap()).unwrap(),
        },
        _ => Call {
            id: item["id"].as_str().unwrap().into(),
            name: item["function"]["name"].as_str().unwrap().into(),
            args: serde_json::from_str(item["function"]["arguments"].as_str().unwrap()).unwrap(),
        },
    }
}

pub(in super::super) fn frames(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|raw| *raw != "[DONE]")
        .map(|raw| serde_json::from_str(raw).unwrap())
        .collect()
}

pub(in super::super) fn calls(text: &str, endpoint: &str, stream: bool) -> Vec<Call> {
    if stream && endpoint == "responses" {
        assert_responses_tool_events(text);
    }
    if !stream || endpoint == "responses" {
        let response = if stream {
            frames(text)
                .into_iter()
                .find(|v| v["type"] == "response.completed")
                .unwrap()["response"]
                .take()
        } else {
            serde_json::from_str(text).unwrap()
        };
        let items = match endpoint {
            "messages" => response["content"].as_array().unwrap(),
            "responses" => response["output"].as_array().unwrap(),
            _ => response["choices"][0]["message"]["tool_calls"]
                .as_array()
                .unwrap(),
        };
        return items
            .iter()
            .filter(|item| match endpoint {
                "messages" => item["type"] == "tool_use",
                "responses" => item["type"] == "function_call",
                _ => true,
            })
            .map(|v| from_item(v, endpoint))
            .collect();
    }
    let mut pending = BTreeMap::<usize, (String, String, String)>::new();
    for frame in frames(text) {
        if endpoint == "messages" {
            let index = frame["index"].as_u64().unwrap_or(0) as usize;
            if frame["type"] == "content_block_start"
                && frame["content_block"]["type"] == "tool_use"
            {
                let item = &frame["content_block"];
                assert!(pending
                    .insert(
                        index,
                        (
                            item["id"].as_str().unwrap().into(),
                            item["name"].as_str().unwrap().into(),
                            String::new()
                        )
                    )
                    .is_none());
            } else if frame["delta"]["type"] == "input_json_delta" {
                pending
                    .get_mut(&index)
                    .unwrap()
                    .2
                    .push_str(frame["delta"]["partial_json"].as_str().unwrap());
            }
        } else if let Some(items) = frame["choices"][0]["delta"]["tool_calls"].as_array() {
            for item in items {
                let index = item["index"].as_u64().unwrap() as usize;
                let entry = pending.entry(index).or_default();
                if let Some(id) = item["id"].as_str() {
                    entry.0.push_str(id);
                }
                if let Some(name) = item["function"]["name"].as_str() {
                    entry.1.push_str(name);
                }
                if let Some(args) = item["function"]["arguments"].as_str() {
                    entry.2.push_str(args);
                }
            }
        }
    }
    pending
        .into_values()
        .map(|(id, name, args)| Call {
            id,
            name,
            args: serde_json::from_str(&args).unwrap(),
        })
        .collect()
}

fn assert_responses_tool_events(text: &str) {
    let events = frames(text);
    let mut items = BTreeMap::<usize, (Value, String, bool)>::new();
    for event in &events {
        let kind = event["type"].as_str().unwrap();
        if kind == "response.output_item.added" && event["item"]["type"] == "function_call" {
            assert!(items
                .insert(
                    event["output_index"].as_u64().unwrap() as usize,
                    (event["item"].clone(), String::new(), false)
                )
                .is_none());
        } else if matches!(
            kind,
            "response.function_call_arguments.delta" | "response.function_call_arguments.done"
        ) {
            let entry = items
                .get_mut(&(event["output_index"].as_u64().unwrap() as usize))
                .unwrap();
            assert_eq!(event["item_id"], entry.0["id"]);
            assert!(!entry.2, "arguments after done");
            if kind.ends_with(".delta") {
                entry.1.push_str(event["delta"].as_str().unwrap());
            } else {
                assert_eq!(event["arguments"], entry.1);
                entry.2 = true;
            }
        } else if kind == "response.output_item.done" && event["item"]["type"] == "function_call" {
            let entry = items
                .get(&(event["output_index"].as_u64().unwrap() as usize))
                .unwrap();
            assert!(entry.2);
            for key in ["id", "call_id", "name"] {
                assert_eq!(event["item"][key], entry.0[key]);
            }
            assert_eq!(event["item"]["arguments"], entry.1);
        }
    }
    let terminals: Vec<_> = events
        .iter()
        .filter(|v| v["type"] == "response.completed")
        .collect();
    assert_eq!(terminals.len(), 1);
    let output = terminals[0]["response"]["output"].as_array().unwrap();
    assert_eq!(
        items.len(),
        output
            .iter()
            .filter(|v| v["type"] == "function_call")
            .count()
    );
    for (index, (item, arguments, done)) in items {
        assert!(done);
        for key in ["id", "call_id", "name"] {
            assert_eq!(output[index][key], item[key]);
        }
        assert_eq!(output[index]["arguments"], arguments);
    }
}

pub(super) fn schema() -> Value {
    json!({"type":"object","properties":{"city":{"type":"string"},"nested":{"type":"array","items":{"type":"integer"}}},"required":["city"]})
}

pub(in super::super) fn request_body(endpoint: &str, stream: bool) -> Value {
    let mut request = body(endpoint, stream);
    let function = json!({"name":"lookup","description":"合成工具","parameters":schema()});
    request["tools"] = match endpoint {
        "messages" => json!([{"name":"lookup","description":"合成工具","input_schema":schema()}]),
        "responses" => {
            json!([{"type":"function","name":"lookup","description":"合成工具","parameters":schema()}])
        }
        _ => json!([{"type":"function","function":function}]),
    };
    request["tool_choice"] = match endpoint {
        "messages" => json!({"type":"tool","name":"lookup"}),
        "responses" => json!({"type":"function","name":"lookup"}),
        _ => json!({"type":"function","function":{"name":"lookup"}}),
    };
    request
}

pub(in super::super) fn followup(endpoint: &str, stream: bool, calls: &[Call]) -> Value {
    let mut request = request_body(endpoint, stream);
    request["tool_choice"] = if endpoint == "messages" {
        json!({"type":"auto"})
    } else {
        json!("auto")
    };
    let results: Vec<_> = calls
        .iter()
        .enumerate()
        .map(|(i, _)| json!({"ordinal":i,"city":"杭州"}))
        .collect();
    match endpoint {
        "messages" => {
            let tools: Vec<_> = calls
                .iter()
                .map(|c| json!({"type":"tool_use","id":c.id,"name":c.name,"input":c.args}))
                .collect();
            let outputs: Vec<_> = calls.iter().zip(&results).map(|(c,r)| json!({"type":"tool_result","tool_use_id":c.id,"content":r.to_string()})).collect();
            request["messages"] = json!([{"role":"user","content":"合成问题"},{"role":"assistant","content":tools},{"role":"user","content":outputs}]);
        }
        "responses" => {
            let mut input = vec![json!({"role":"user","content":"合成问题"})];
            input.extend(calls.iter().map(|c| json!({"type":"function_call","call_id":c.id,"name":c.name,"arguments":c.args.to_string()})));
            input.extend(calls.iter().zip(&results).map(|(c,r)| json!({"type":"function_call_output","call_id":c.id,"output":r.to_string()})));
            request["input"] = json!(input);
        }
        _ => {
            let tools: Vec<_> = calls.iter().map(|c| json!({"id":c.id,"type":"function","function":{"name":c.name,"arguments":c.args.to_string()}})).collect();
            let mut messages = vec![
                json!({"role":"user","content":"合成问题"}),
                json!({"role":"assistant","content":null,"tool_calls":tools}),
            ];
            messages.extend(
                calls.iter().zip(&results).map(
                    |(c, r)| json!({"role":"tool","tool_call_id":c.id,"content":r.to_string()}),
                ),
            );
            request["messages"] = json!(messages);
        }
    }
    // Clients may finish parallel work out of order; v1 must correlate locally and reorder it.
    match endpoint {
        "messages" => request["messages"][2]["content"]
            .as_array_mut()
            .unwrap()
            .reverse(),
        "responses" => request["input"].as_array_mut().unwrap()[1 + calls.len()..].reverse(),
        _ => request["messages"].as_array_mut().unwrap()[2..].reverse(),
    }
    request
}
