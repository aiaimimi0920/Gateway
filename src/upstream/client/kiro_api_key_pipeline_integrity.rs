//! Key must reject semantically corrupt frames and lossy unsupported history before claiming success.
use super::*;

#[test]
fn kiro_api_key_pipeline_malformed_events_and_unfinished_tools_never_complete() {
    run(invalid_frames);
}
async fn invalid_frames() {
    let malformed = wire::frame(
        &[
            (":message-type", "event"),
            (":event-type", "assistantResponseEvent"),
        ],
        b"{",
    );
    let partial = wire::event(
        "toolUseEvent",
        json!({"name":"get_weather","toolUseId":"call","input":"{\"city\":","stop":false}),
    );
    let invalid = wire::event(
        "toolUseEvent",
        json!({"name":"get_weather","toolUseId":"call","input":"{\"city\":","stop":true}),
    );
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            for tail in [
                malformed.clone(),
                wire::event("toolUseEvent", json!({})),
                partial.clone(),
                invalid.clone(),
            ] {
                let server =
                    wire::Server::stream([wire::event_frame("VISIBLE"), tail].concat(), false)
                        .await;
                let fixture = Fixture::new(&server).await;
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, body(endpoint, stream), true).await;
                    let status = response.status();
                    let (text, failed) = drain(response.into_body()).await;
                    assert!(
                        status != StatusCode::OK || failed || text.contains("error"),
                        "{endpoint} stream={stream}: {text}"
                    );
                    assert!(
                        !text.contains("response.completed")
                            && !text.contains("message_stop")
                            && !text.contains("[DONE]"),
                        "{text}"
                    );
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "failed");
                    assert!(row.total_tokens.is_none());
                    assert_wire(&server);
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    }
}

fn invalid_requests(endpoint: &str) -> Vec<Value> {
    let mut requests = Vec::new();
    let mut trailing = body(endpoint, false);
    let mut remote = body(endpoint, false);
    let mut orphan = body(endpoint, false);
    let mut bad_arguments = super::tools::request(endpoint, false);
    match endpoint {
        "responses" => {
            trailing["input"] =
                json!([{"role":"user","content":"before"},{"role":"assistant","content":"prefix"}]);
            remote["input"] = json!([{"role":"user","content":[{"type":"input_text","text":"合成问题"},{"type":"input_image","image_url":"https://foreign.invalid/image.png"}]}]);
            orphan["input"] = json!([{"type":"function_call_output","call_id":"orphan","output":"private-result"}]);
            bad_arguments["input"][1]["arguments"] = json!("{broken");
            let mut previous = body(endpoint, false);
            previous["previous_response_id"] = json!("unresolvable-previous");
            requests.push(previous);
        }
        "messages" => {
            trailing["messages"] =
                json!([{"role":"user","content":"before"},{"role":"assistant","content":"prefix"}]);
            remote["messages"] = json!([{"role":"user","content":[{"type":"text","text":"合成问题"},{"type":"image","source":{"type":"url","url":"https://foreign.invalid/image.png"}}]}]);
            orphan["messages"] = json!([{"role":"user","content":[{"type":"tool_result","tool_use_id":"orphan","content":"private-result"}]}]);
            bad_arguments["messages"][1]["content"][0]["input"] = Value::Null;
        }
        _ => {
            trailing["messages"] =
                json!([{"role":"user","content":"before"},{"role":"assistant","content":"prefix"}]);
            remote["messages"] = json!([{"role":"user","content":[{"type":"text","text":"合成问题"},{"type":"image_url","image_url":{"url":"https://foreign.invalid/image.png"}}]}]);
            orphan["messages"] =
                json!([{"role":"tool","tool_call_id":"orphan","content":"private-result"}]);
            bad_arguments["messages"][2]["tool_calls"][0]["function"]["arguments"] =
                json!("{broken");
        }
    }
    requests.extend([trailing, remote, orphan, bad_arguments]);
    requests
}

#[test]
fn kiro_api_key_pipeline_lossy_history_and_remote_images_are_rejected_before_send() {
    run(invalid_history);
}
async fn invalid_history() {
    for endpoint in ["chat/completions", "messages", "responses"] {
        for mut request in invalid_requests(endpoint) {
            for stream in [false, true] {
                request["stream"] = json!(stream);
                let server = wire::Server::stream(wire::event_frame("unreachable"), false).await;
                let fixture = Fixture::new(&server).await;
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, request.clone(), true).await;
                    let status = response.status();
                    let (text, _) = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::BAD_REQUEST, "{endpoint}: {text}");
                    assert!(!text.contains("unreachable") && !text.contains("private-result"));
                    assert_eq!(server.attempts(), 0);
                    assert_eq!(fixture.audit().await.status, "failed");
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    }
}
