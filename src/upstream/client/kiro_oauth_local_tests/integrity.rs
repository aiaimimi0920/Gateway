//! Normal OAuth login and public TCP: ambiguous native output cannot settle as success.
use super::*;
use futures::StreamExt;

#[test]
fn kiro_oauth_runtime_integrity_public_errors_refund_without_success_terminal_or_replay() {
    run(180, ambiguous);
}
async fn ambiguous() {
    let malformed = [
        wire::frame(
            &[
                (":message-type", "event"),
                (":event-type", "assistantResponseEvent"),
            ],
            br#"{"content":"x","content":"y"}"#,
        ),
        wire::frame(
            &[
                (":message-type", "event"),
                (":event-type", "assistantResponseEvent"),
                (":event-type", "ignored"),
            ],
            br#"{"content":"x"}"#,
        ),
        wire::frame(
            &[(":event-type", "assistantResponseEvent")],
            br#"{"content":"x"}"#,
        ),
    ];
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            for bad in &malformed {
                let bytes = [wire::event_frame("visible"), bad.clone()].concat();
                let upstream =
                    wire::Server::start(200, "application/vnd.amazon.eventstream", bytes).await;
                let fixture = Fixture::new(&upstream).await;
                let response = fixture
                    .server
                    .try_open(endpoint, &fixture.token, body(endpoint, stream))
                    .await;
                match response {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let mut reader = response.bytes_stream();
                        let mut text = Vec::new();
                        let mut failed = false;
                        while let Some(chunk) = reader.next().await {
                            match chunk {
                                Ok(chunk) => {
                                    assert!(text.len() + chunk.len() <= 65536);
                                    text.extend_from_slice(&chunk);
                                }
                                Err(_) => {
                                    failed = true;
                                    break;
                                }
                            }
                        }
                        let text = String::from_utf8(text).unwrap();
                        fixture::assert_redacted(&json!(text));
                        assert!(status != 200 || failed || text.contains("error"), "{text}");
                        assert!(
                            !text.contains("response.completed")
                                && !text.contains("message_stop")
                                && !text.contains("[DONE]"),
                            "{text}"
                        );
                    }
                    Err(_) => assert!(stream, "buffered request must have an error response"),
                }
                fixture.assert_identity(&fixture.audits(1).await, "failed");
                assert_eq!(remaining(&fixture.server, &fixture.key_id).await, 10);
                assert_wire(&upstream, 1);
                fixture.stop().await;
                upstream.stop().await;
            }
        }
    }
}

#[test]
fn kiro_oauth_runtime_integrity_public_empty_tools_have_consistent_object_arguments() {
    run(120, empty_tools);
}
async fn empty_tools() {
    let frames: Vec<_> = [
        json!({"name":"read","toolUseId":"a"}),
        json!({"name":"other","toolUseId":"b","input":" "}),
        json!({"toolUseId":"a","input":"\u{2003}"}),
        json!({"toolUseId":"b","input":"\t"}),
        json!({"toolUseId":"a","stop":true}),
        json!({"toolUseId":"b","stop":true}),
    ]
    .iter()
    .map(|value| wire::event("toolUseEvent", value.clone()))
    .collect();
    let upstream = wire::Server::stream(frames.concat(), false).await;
    let fixture = Fixture::new(&upstream).await;
    let schema = json!({"type":"object","properties":{}});
    let mut calls = 0;
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            let mut request = body(endpoint, stream);
            request["tools"] = json!(["read", "other"].map(|name| match endpoint {
                "messages" => json!({"name":name,"input_schema":schema}),
                "responses" => json!({"type":"function","name":name,"parameters":schema}),
                _ => json!({"type":"function","function":{"name":name,"parameters":schema}}),
            }));
            let response = fixture.server.open(endpoint, &fixture.token, request).await;
            assert_eq!(response.status().as_u16(), 200);
            let text = response.text().await.unwrap();
            assert_tools(endpoint, stream, &text);
            calls += 1;
            fixture.assert_identity(&fixture.audits(calls).await, "completed");
            assert_eq!(
                remaining(&fixture.server, &fixture.key_id).await,
                10 - calls as i64
            );
        }
    }
    assert_wire(&upstream, 6);
    fixture.stop().await;
    upstream.stop().await;
}

fn assert_tools(endpoint: &str, stream: bool, text: &str) {
    let object = |value: &Value| {
        let parsed = value
            .as_str()
            .map(|s| serde_json::from_str::<Value>(s).unwrap())
            .unwrap_or_else(|| value.clone());
        assert_eq!(parsed, json!({}));
    };
    if !stream {
        let value: Value = serde_json::from_str(text).unwrap();
        let tools: Vec<_> = match endpoint {
            "messages" => value["content"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| t["type"] == "tool_use")
                .map(|t| &t["input"])
                .collect(),
            "responses" => value["output"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| t["type"] == "function_call")
                .map(|t| &t["arguments"])
                .collect(),
            _ => value["choices"][0]["message"]["tool_calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| &t["function"]["arguments"])
                .collect(),
        };
        assert_eq!(tools.len(), 2, "{text}");
        for value in tools {
            object(value);
        }
        return;
    }
    let events: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let mut arguments = std::collections::BTreeMap::<u64, String>::new();
    match endpoint {
        "messages" => {
            let mut identities = std::collections::BTreeSet::new();
            for start in events.iter().filter(|v| {
                v["type"] == "content_block_start" && v["content_block"]["type"] == "tool_use"
            }) {
                let index = start["index"].as_u64().unwrap();
                identities.insert((
                    start["content_block"]["id"].as_str().unwrap(),
                    start["content_block"]["name"].as_str().unwrap(),
                ));
                let deltas: String = events
                    .iter()
                    .filter(|v| {
                        v["type"] == "content_block_delta"
                            && v["index"] == index
                            && v["delta"]["type"] == "input_json_delta"
                    })
                    .map(|v| v["delta"]["partial_json"].as_str().unwrap())
                    .collect();
                let input = if deltas.is_empty() {
                    start["content_block"]["input"].to_string()
                } else {
                    deltas
                };
                assert_eq!(
                    events
                        .iter()
                        .filter(|v| v["type"] == "content_block_stop" && v["index"] == index)
                        .count(),
                    1
                );
                assert!(arguments.insert(index, input).is_none());
            }
            assert_eq!(identities, [("a", "read"), ("b", "other")].into());
        }
        "responses" => {
            let terminal: Vec<_> = events
                .iter()
                .filter(|v| v["type"] == "response.completed")
                .collect();
            assert_eq!(terminal.len(), 1);
            let output: Vec<_> = terminal[0]["response"]["output"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "function_call")
                .collect();
            assert_eq!(output.len(), 2);
            let identities: std::collections::BTreeSet<_> = output
                .iter()
                .map(|v| (v["call_id"].as_str().unwrap(), v["name"].as_str().unwrap()))
                .collect();
            assert_eq!(identities, [("a", "read"), ("b", "other")].into());
            for done in events
                .iter()
                .filter(|v| v["type"] == "response.function_call_arguments.done")
            {
                let index = done["output_index"].as_u64().unwrap();
                let deltas: String = events
                    .iter()
                    .filter(|v| {
                        v["type"] == "response.function_call_arguments.delta"
                            && v["output_index"] == index
                    })
                    .map(|v| v["delta"].as_str().unwrap())
                    .collect();
                assert_eq!(deltas, done["arguments"].as_str().unwrap());
                let item = output.iter().find(|v| v["id"] == done["item_id"]).unwrap();
                assert_eq!(item["arguments"], done["arguments"]);
                assert!(arguments.insert(index, deltas).is_none());
            }
        }
        _ => {
            let mut identities = std::collections::BTreeSet::new();
            for value in &events {
                if let Some(tools) = value["choices"][0]["delta"]["tool_calls"].as_array() {
                    for tool in tools {
                        if let Some(id) = tool["id"].as_str() {
                            identities.insert((id, tool["function"]["name"].as_str().unwrap()));
                        }
                        arguments
                            .entry(tool["index"].as_u64().unwrap())
                            .or_default()
                            .push_str(tool["function"]["arguments"].as_str().unwrap());
                    }
                }
            }
            assert_eq!(identities, [("a", "read"), ("b", "other")].into());
        }
    }
    assert_eq!(arguments.len(), 2, "{text}");
    for value in arguments.values() {
        object(&json!(value));
    }
    assert!(text.contains(match endpoint {
        "messages" => "message_stop",
        "responses" => "response.completed",
        _ => "[DONE]",
    }));
}
