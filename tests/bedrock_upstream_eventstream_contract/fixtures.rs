//! Synthetic AWS wire frames: event identity is in headers, never a JSON wrapper.
use crc::{Crc, CRC_32_ISO_HDLC};
use serde_json::{json, Value};

pub fn frame(headers: &[(&str, &str)], payload: &Value) -> Vec<u8> {
    let mut encoded = Vec::new();
    for (name, value) in headers {
        encoded.push(name.len() as u8);
        encoded.extend_from_slice(name.as_bytes());
        encoded.push(7);
        encoded.extend_from_slice(&(value.len() as u16).to_be_bytes());
        encoded.extend_from_slice(value.as_bytes());
    }
    raw_frame(&encoded, &serde_json::to_vec(payload).unwrap())
}

pub fn raw_frame(encoded: &[u8], body: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&((16 + encoded.len() + body.len()) as u32).to_be_bytes());
    output.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    output.extend_from_slice(&crc.checksum(&output).to_be_bytes());
    output.extend_from_slice(encoded);
    output.extend_from_slice(body);
    output.extend_from_slice(&crc.checksum(&output).to_be_bytes());
    output
}

pub fn event(kind: &str, body: Value) -> Vec<u8> {
    frame(
        &[
            (":message-type", "event"),
            (":event-type", kind),
            (":content-type", "application/json"),
        ],
        &body,
    )
}

pub fn sequence() -> Vec<Vec<u8>> {
    vec![
        event("messageStart", json!({"role":"assistant"})),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":"你好"}}),
        ),
        event("contentBlockStop", json!({"contentBlockIndex":0})),
        event(
            "contentBlockStart",
            json!({"contentBlockIndex":1,"start":{"toolUse":{"toolUseId":"call-1","name":"lookup"}}}),
        ),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":"{\"city\":"}}}),
        ),
        event(
            "contentBlockDelta",
            json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":" \"北京\"}"}}}),
        ),
        event("contentBlockStop", json!({"contentBlockIndex":1})),
        event("messageStop", json!({"stopReason":"tool_use"})),
        event(
            "metadata",
            json!({"usage":{"inputTokens":5,"outputTokens":8,"totalTokens":13,"cacheReadInputTokens":2,"cacheWriteInputTokens":1},"metrics":{"latencyMs":1}}),
        ),
    ]
}

pub fn response(frames: Vec<Vec<u8>>) -> rquest::Response {
    axum::http::Response::builder()
        .header("content-type", "application/vnd.amazon.eventstream")
        .body(frames.concat())
        .unwrap()
        .into()
}

pub fn repair_crc(frame: &mut [u8]) {
    let end = frame.len() - 4;
    let checksum = Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(&frame[..end]);
    frame[end..].copy_from_slice(&checksum.to_be_bytes());
}
