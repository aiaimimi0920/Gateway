//! Synthetic completions only. Prompts select cases; no network, prompt or token logging.
use serde_json::{json, Value};

pub fn idle() -> Value {
    json!({"id":"terminal", "timestamp":"2026-11-13T00:00:00Z", "type":"session.idle", "data":{}})
}

pub fn response(request: &Value) -> Option<Value> {
    let prompt = request["params"]["prompt"].as_str().unwrap();
    let mut event = json!({"id":"reply", "timestamp":"2026-11-13T00:00:00Z",
        "type":"assistant.message", "data":{"messageId":"reply", "content":"synthetic answer"}});
    match prompt {
        "hang" => return None,
        "denied" => {
            return Some(json!({"jsonrpc":"2.0", "id":request["id"],
            "error":{"code":-32000,"message":"synthetic-token must never escape"}}))
        }
        "missing" => event = Value::Null,
        "wrong-agent" => event["agentId"] = json!("unexpected-child"),
        "wrong-type" => event["type"] = json!("assistant.message_delta"),
        "malformed" => event["data"] = json!({"content":17}),
        "multipart" => event["data"]["chunkCount"] = json!(2),
        "tool" => {
            event["data"]["toolRequests"] = json!([{"toolCallId":"call", "name":"unexpected"}])
        }
        "echo" => event["data"]["content"] = json!("synthetic-token"),
        "oversize" => event["data"]["content"] = json!("x".repeat(1024 * 1024 + 1)),
        "negative" => event["data"]["outputTokens"] = json!(-1),
        "known-usage" => event["data"]["outputTokens"] = json!(3),
        "ok" | "aborted" => {}
        _ => panic!("unknown synthetic text case"),
    }
    Some(json!({"jsonrpc":"2.0","id":request["id"],"result":{
        "messageId":"input", "aborted":prompt == "aborted", "response":event,
        "completionEventId":"terminal"
    }}))
}
