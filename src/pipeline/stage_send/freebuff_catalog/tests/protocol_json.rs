//! Public JSON protocols share the original purchase, signed run, cost and Local settlement owner.
use super::*;
mod boundaries;
mod projection;
const RESPONSES: &str = "/v1/responses";
const MESSAGES: &str = "/v1/messages";
fn input(path: &str) -> Value {
    if path == RESPONSES {
        json!({"model":"open","instructions":"caller policy","input":[
            {"role":"user","content":[{"type":"input_text","text":"first"}]},
            {"role":"developer","content":"in-sequence policy"},
            {"type":"function_call","call_id":"c1","name":"lookup","arguments":"{ \"s\":\"&amp;\" }"},
            {"type":"function_call_output","call_id":"c1","output":"caller result"},
            {"role":"user","content":"next"}],
            "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}],
            "max_output_tokens":16,"reasoning":{"effort":"high"}})
    } else {
        json!({"model":"open","system":[{"type":"text","text":"caller policy"},{"type":"text","text":"second policy"}],
            "messages":[{"role":"user","content":"first"},
            {"role":"assistant","content":[{"type":"text","text":"before"},{"type":"tool_use","id":"c1","name":"lookup","input":{"s":"&amp;"}}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"c1","content":[{"type":"text","text":"caller result"}]},{"type":"text","text":"next"}]}],
            "tools":[{"name":"lookup","input_schema":{"type":"object"}}],"tool_choice":{"type":"any","disable_parallel_tool_use":true},"max_tokens":16})
    }
}
async fn success(path: &'static str) {
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let grant = host.grant().await;
    let task = host.public_at(path, input(path));
    let view = host.pending(&grant, &task).await;
    assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 0);
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
    let (status, body) = task.await.unwrap();
    assert_eq!(status, 200, "{body}");
    if path == RESPONSES {
        assert_eq!(body["object"], "response");
        assert_eq!(body["status"], "completed");
        assert_eq!(body["output"][0]["content"][0]["text"], "native answer");
    } else {
        assert_eq!(body["type"], "message");
        assert_eq!(body["stop_reason"], "end_turn");
        assert_eq!(body["content"][0]["text"], "native answer");
        assert!(body["content"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["type"] != "thinking"));
    }
    assert_eq!(body["usage"]["input_tokens"], 3);
    assert_eq!(body["usage"]["output_tokens"], 4);
    assert_eq!(
        body["gateway_freebuff"]["message"]["reasoning_content"],
        "native reason"
    );
    assert_eq!(
        body["gateway_freebuff"]["message"]["reasoning_details"],
        json!([{"opaque":"keep"}])
    );
    assert_eq!(
        body["gateway_freebuff"]["completion"]["usage"],
        peer::completion()["usage"]
    );
    let wire = peer
        .hits
        .lock()
        .unwrap()
        .iter()
        .find(|(_, p, _, _)| p == "/api/v1/chat/completions")
        .unwrap()
        .3
        .clone();
    assert_eq!(
        wire["messages"][0],
        json!({"role":"system","content":"caller policy"})
    );
    if path == RESPONSES {
        assert_eq!(
            wire["messages"][2],
            json!({"role":"developer","content":"in-sequence policy"})
        );
        assert_eq!(
            wire["messages"][3]["tool_calls"][0]["function"]["arguments"],
            "{ \"s\":\"&amp;\" }"
        );
        assert_eq!(wire["reasoning_effort"], "high");
    } else {
        assert_eq!(
            wire["messages"][1],
            json!({"role":"system","content":"second policy"})
        );
        assert_eq!(wire["messages"][5]["role"], "tool");
        assert_eq!(
            wire["messages"][6]["content"],
            json!([{"type":"text","text":"next"}])
        );
        assert_eq!(wire["tool_choice"], "required");
        assert_eq!(wire["parallel_tool_calls"], false);
    }
    assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
    assert_eq!(peer.finishes()[0]["totalCredits"], 13);
    assert_eq!(peer.finishes()[0]["status"], "completed");
    assert_eq!(claims(&host).await, 1);
    settled(&host, "completed", 9).await;
    host.close().await;
    peer.server.close().await;
}
#[tokio::test]
async fn freebuff_public_responses_json_preserves_native_data_and_original_owner() {
    success(RESPONSES).await;
}
#[tokio::test]
async fn freebuff_public_messages_json_preserves_native_data_and_original_owner() {
    success(MESSAGES).await;
}
pub(super) fn completion(mode: &str) -> Option<Value> {
    let mut body = peer::completion();
    match mode {
        "json_multiple" => {
            let second = body["choices"][0].clone();
            body["choices"].as_array_mut().unwrap().push(second);
        }
        "json_partial" => {
            body["usage"]
                .as_object_mut()
                .unwrap()
                .remove("completion_tokens");
        }
        "json_tools" | "json_array_args" | "json_missing_id" => {
            body["choices"][0]["message"]["content"] =
                json!("literal <tool_calls>not executable</tool_calls>");
            body["choices"][0]["message"]["tool_calls"] = json!([{"id":"out-1","type":"function","function":{"name":"lookup","arguments":"{ \"s\": \"&amp;\" }"},"extra_content":{"opaque":"keep"}}]);
            body["choices"][0]["finish_reason"] = json!("tool_calls");
            if mode == "json_array_args" {
                body["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] =
                    json!("[]");
            }
            if mode == "json_missing_id" {
                body["choices"][0]["message"]["tool_calls"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("id");
            }
        }
        _ => return None,
    }
    if mode == "json_multiple" {
        body["choices"][1]["index"] = json!(1);
    }
    Some(body)
}
