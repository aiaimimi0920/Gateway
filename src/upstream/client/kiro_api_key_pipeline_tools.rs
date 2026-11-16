//! Public normalizers preserve tool history, results, data images and tool response identity.
use super::*;
const IMAGE: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==";

pub(super) fn request(endpoint: &str, stream: bool) -> Value {
    let schema = json!({"type":"object","properties":{"city":{"type":"string"}}});
    let image = format!("data:image/png;base64,{IMAGE}");
    match endpoint {
        "messages" => {
            json!({"model":"public-alias","system":"rules","max_tokens":32,"stream":stream,
            "tools":[{"name":"get_weather","input_schema":schema}],
            "messages":[{"role":"user","content":"before"},
                {"role":"assistant","content":[{"type":"tool_use","id":"prior","name":"get_weather","input":{"city":"北京"}}]},
                {"role":"user","content":[{"type":"tool_result","tool_use_id":"prior","content":"RESULT"},
                    {"type":"text","text":"合成问题"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":IMAGE}}]}]})
        }
        "responses" => json!({"model":"public-alias","instructions":"rules","stream":stream,
            "tools":[{"type":"function","name":"get_weather","parameters":schema}],
            "input":[{"role":"user","content":"before"},
                {"type":"function_call","call_id":"prior","name":"get_weather","arguments":"{\"city\":\"北京\"}"},
                {"type":"function_call_output","call_id":"prior","output":"RESULT"},
                {"role":"user","content":[{"type":"input_text","text":"合成问题"},{"type":"input_image","image_url":image}]}]}),
        _ => json!({"model":"public-alias","stream":stream,
            "tools":[{"type":"function","function":{"name":"get_weather","parameters":schema}}],
            "messages":[{"role":"system","content":"rules"},{"role":"user","content":"before"},
                {"role":"assistant","content":null,"tool_calls":[{"id":"prior","type":"function","function":{"name":"get_weather","arguments":"{\"city\":\"北京\"}"}}]},
                {"role":"tool","tool_call_id":"prior","content":"RESULT"},
                {"role":"user","content":[{"type":"text","text":"合成问题"},{"type":"image_url","image_url":{"url":image}}]}]}),
    }
}

#[test]
fn kiro_api_key_pipeline_tools_images_and_tool_results_reach_real_wire_and_response() {
    run(tools);
}
async fn tools() {
    let output = [
        wire::event(
            "toolUseEvent",
            json!({"name":"get_weather","toolUseId":"next","input":"{\"city\":","stop":false}),
        ),
        wire::event(
            "toolUseEvent",
            json!({"name":"get_weather","toolUseId":"next","input":"\"杭州\"}","stop":true}),
        ),
    ]
    .concat();
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            let server = wire::Server::stream(output.clone(), false).await;
            let fixture = Fixture::new(&server).await;
            wire::within(&server.origin, async {
                let response = fixture
                    .open(endpoint, request(endpoint, stream), true)
                    .await;
                let status = response.status();
                let (text, failed) = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{endpoint} stream={stream}: {text}");
                assert!(!failed, "{text}");
                assert!(
                    text.contains("get_weather") && text.contains("next") && text.contains("杭州"),
                    "{text}"
                );
                if !stream {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    let (call, arguments) = match endpoint {
                        "messages" => (&value["content"][0], value["content"][0]["input"].clone()),
                        "responses" => (
                            &value["output"][0],
                            serde_json::from_str(value["output"][0]["arguments"].as_str().unwrap())
                                .unwrap(),
                        ),
                        _ => (
                            &value["choices"][0]["message"]["tool_calls"][0],
                            serde_json::from_str(
                                value["choices"][0]["message"]["tool_calls"][0]["function"]
                                    ["arguments"]
                                    .as_str()
                                    .unwrap(),
                            )
                            .unwrap(),
                        ),
                    };
                    assert_eq!(arguments, json!({"city":"杭州"}));
                    assert!(call.to_string().contains("get_weather"));
                }
                assert_wire(&server);
                {
                    let calls = server.calls.lock().unwrap();
                    let state = &calls[0].1["conversationState"];
                    let current = &state["currentMessage"]["userInputMessage"];
                    assert_eq!(current["images"][0]["source"]["bytes"], IMAGE);
                    let context = &current["userInputMessageContext"];
                    assert_eq!(
                        context["tools"][0]["toolSpecification"]["name"],
                        "get_weather"
                    );
                    assert_eq!(context["toolResults"][0]["toolUseId"], "prior");
                    assert_eq!(context["toolResults"][0]["content"][0]["text"], "RESULT");
                    assert!(state["history"].to_string().contains("prior"));
                }
                let audit = fixture.audit().await;
                assert_eq!(audit.status, "completed");
                assert_identity(&audit);
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    }
}
