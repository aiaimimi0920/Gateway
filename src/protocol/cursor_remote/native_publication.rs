//! Bounded native phase delivery validation. A tool pause is claimable only
//! after the complete protocol terminal reaches the HTTP body consumer.
use crate::protocol::{
    canonical::ProtocolFamily,
    stream_decode::{BoundedSseDecoder, DecodeStep},
};
use bytes::Bytes;
use serde_json::{json, Value};
use std::io;

fn invalid() -> io::Error {
    io::Error::other("Cursor native tool phase was not completely delivered")
}

pub(crate) fn validate(family: ProtocolFamily, bytes: Bytes, sse: bool) -> Result<(), io::Error> {
    if !sse {
        return validate_json(
            family,
            &serde_json::from_slice(&bytes).map_err(|_| invalid())?,
        );
    }
    let mut decoder = BoundedSseDecoder::new_strict(1024 * 1024);
    decoder.push_chunk(bytes);
    let mut events = Vec::new();
    loop {
        let frame = match decoder.decode_next() {
            DecodeStep::Frame(frame) => frame,
            DecodeStep::NeedInput => break,
            DecodeStep::Error(_) => return Err(invalid()),
        };
        let value: Value = serde_json::from_str(&frame.data).map_err(|_| invalid())?;
        let expected = if family == ProtocolFamily::CohereChat {
            Some(value["type"].as_str().ok_or_else(invalid)?)
        } else {
            None
        };
        if frame.event_name.as_deref() != expected || events.len() >= 8 {
            return Err(invalid());
        }
        events.push(value);
    }
    decoder.finish_strict().map_err(|_| invalid())?;
    if family == ProtocolFamily::GeminiGenerateContent {
        if events.len() != 1 {
            return Err(invalid());
        }
        return validate_json(family, &events[0]);
    }
    let first = events.first().ok_or_else(invalid)?;
    let mut value =
        json!({"id":first["id"],"message":{"role":"assistant"},"finish_reason":"TOOL_CALL"});
    for event in &events {
        match event["type"].as_str() {
            Some("content-delta") => {
                value["message"]["content"] =
                    json!([{"type":"text","text":event["delta"]["message"]["content"]["text"]}]);
            }
            Some("tool-call-start") => {
                value["message"]["tool_calls"] = json!([event["delta"]["message"]["tool_calls"]]);
            }
            Some("tool-call-delta") if value["message"]["tool_calls"].is_array() => {
                value["message"]["tool_calls"][0]["function"]["arguments"] =
                    event["delta"]["message"]["tool_calls"]["function"]["arguments"].clone();
            }
            _ => {}
        }
    }
    validate_json(family, &value)?;
    if events != super::native_output::cohere_events(&value) {
        return Err(invalid());
    }
    Ok(())
}

fn validate_json(family: ProtocolFamily, value: &Value) -> Result<(), io::Error> {
    let (id, name, args) = match family {
        ProtocolFamily::GeminiGenerateContent => {
            let candidates = value["candidates"]
                .as_array()
                .filter(|a| a.len() == 1)
                .ok_or_else(invalid)?;
            let candidate = &candidates[0];
            if candidate["index"] != 0
                || candidate["finishReason"] != "STOP"
                || candidate["content"]["role"] != "model"
                || value.get("error").is_some()
            {
                return Err(invalid());
            }
            let parts = candidate["content"]["parts"]
                .as_array()
                .ok_or_else(invalid)?;
            if parts.is_empty()
                || parts.len() > 2
                || parts.len() == 2
                    && (!parts[0]["text"].is_string()
                        || parts[0].as_object().map(|m| m.len()) != Some(1))
            {
                return Err(invalid());
            }
            let part = parts.last().unwrap();
            let call = &part["functionCall"];
            if part.as_object().map(|m| m.len()) != Some(1) || !call["args"].is_object() {
                return Err(invalid());
            }
            (
                call["id"].as_str(),
                call["name"].as_str(),
                Some(call["args"].clone()),
            )
        }
        ProtocolFamily::CohereChat => {
            if value["id"].as_str().is_none_or(str::is_empty)
                || value["finish_reason"] != "TOOL_CALL"
                || value["message"]["role"] != "assistant"
                || value.get("error").is_some()
            {
                return Err(invalid());
            }
            if let Some(content) = value["message"].get("content") {
                if content.as_array().map(|a| a.len()) != Some(1)
                    || content[0]["type"] != "text"
                    || !content[0]["text"].is_string()
                {
                    return Err(invalid());
                }
            }
            let calls = value["message"]["tool_calls"]
                .as_array()
                .filter(|a| a.len() == 1)
                .ok_or_else(invalid)?;
            let call = &calls[0];
            if call["type"] != "function" {
                return Err(invalid());
            }
            let args = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str::<Value>(a).ok());
            (call["id"].as_str(), call["function"]["name"].as_str(), args)
        }
        _ => return Err(invalid()),
    };
    if id.is_none_or(str::is_empty)
        || name.is_none_or(str::is_empty)
        || args.as_ref().is_none_or(|a| !a.is_object())
    {
        return Err(invalid());
    }
    Ok(())
}
