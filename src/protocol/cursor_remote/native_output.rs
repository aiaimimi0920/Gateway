//! GenerateContent and Cohere output from a verified native phase, without a
//! lossy OpenAI round-trip. Unknown usage/model identity/thinking stay absent.
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayResponse, ProtocolFamily},
};
use bytes::Bytes;
use serde_json::{json, Value};

pub(crate) fn supports(family: ProtocolFamily) -> bool {
    matches!(
        family,
        ProtocolFamily::GeminiGenerateContent | ProtocolFamily::CohereChat
    )
}

pub(crate) fn response(
    family: ProtocolFamily,
    completed: &CanonicalRelayResponse,
) -> Result<Value, GatewayError> {
    if completed.tool_calls.len() > 1 {
        return Err(super::error("worker_protocol"));
    }
    let mut call = None;
    if let Some(tool) = completed.tool_calls.first() {
        let id = tool.id.as_deref().filter(|s| !s.is_empty());
        let name = tool.name.as_deref().filter(|s| !s.is_empty());
        let args = tool.arguments.as_deref();
        if tool.call_type != "function" || id.is_none() || name.is_none() || args.is_none() {
            return Err(super::error("worker_protocol"));
        }
        let object: Value =
            serde_json::from_str(args.unwrap()).map_err(|_| super::error("worker_protocol"))?;
        if !object.is_object() {
            return Err(super::error("worker_protocol"));
        }
        call = Some((id.unwrap(), name.unwrap(), args.unwrap(), object));
    }
    let id = format!("cursor_{}", uuid::Uuid::new_v4().simple());
    match family {
        ProtocolFamily::GeminiGenerateContent => {
            let mut parts = Vec::new();
            if !completed.text.is_empty() || call.is_none() {
                parts.push(json!({"text":completed.text}));
            }
            if let Some((id, name, _, args)) = call {
                parts.push(json!({"functionCall":{"id":id,"name":name,"args":args}}));
            }
            Ok(
                json!({"responseId":id,"candidates":[{"index":0,"content":{"role":"model","parts":parts},"finishReason":"STOP"}]}),
            )
        }
        ProtocolFamily::CohereChat => {
            let mut message = json!({"role":"assistant"});
            if !completed.text.is_empty() || call.is_none() {
                message["content"] = json!([{"type":"text","text":completed.text}]);
            }
            let finish = if let Some((id, name, arguments, _)) = call {
                message["tool_calls"] = json!([{"id":id,"type":"function",
                    "function":{"name":name,"arguments":arguments}}]);
                "TOOL_CALL"
            } else {
                "COMPLETE"
            };
            Ok(json!({"id":id,"message":message,"finish_reason":finish}))
        }
        _ => Err(super::error("worker_protocol")),
    }
}

pub(crate) fn frames(family: ProtocolFamily, value: &Value) -> Vec<Bytes> {
    if family == ProtocolFamily::GeminiGenerateContent {
        return vec![Bytes::from(format!("data: {value}\n\n"))];
    }
    cohere_events(value)
        .into_iter()
        .map(|event| {
            Bytes::from(format!(
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().unwrap()
            ))
        })
        .collect()
}

pub(super) fn cohere_events(value: &Value) -> Vec<Value> {
    let message = &value["message"];
    let mut events = vec![json!({"type":"message-start","id":value["id"],
        "delta":{"message":{"role":"assistant"}}})];
    if let Some(content) = message.get("content") {
        events.extend([
            json!({"type":"content-start","index":0,"delta":{"message":{"content":{"type":"text","text":""}}}}),
            json!({"type":"content-delta","index":0,"delta":{"message":{"content":{"text":content[0]["text"]}}}}),
            json!({"type":"content-end","index":0}),
        ]);
    }
    if let Some(calls) = message.get("tool_calls") {
        let call = &calls[0];
        events.extend([
            json!({"type":"tool-call-start","index":0,"delta":{"message":{"tool_calls":{
                "id":call["id"],"type":"function","function":{"name":call["function"]["name"],"arguments":""}}}}}),
            json!({"type":"tool-call-delta","index":0,"delta":{"message":{"tool_calls":{
                "function":{"arguments":call["function"]["arguments"]}}}}}),
            json!({"type":"tool-call-end","index":0}),
        ]);
    }
    events.push(json!({"type":"message-end","delta":{"finish_reason":value["finish_reason"]}}));
    events
}

#[cfg(test)]
#[path = "native_output_tests.rs"]
mod tests;
