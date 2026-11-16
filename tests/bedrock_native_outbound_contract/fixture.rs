use std::collections::BTreeMap;

use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};
use futures::{stream, StreamExt};
use neuro_gateway::protocol::{bedrock_converse, stream_error::StreamError};
use serde_json::Value;

pub fn sse(frames: &[Value]) -> Vec<u8> {
    let mut wire = String::new();
    for frame in frames {
        wire.push_str(&format!("data: {frame}\n\n"));
    }
    wire.push_str("data: [DONE]\n\n");
    wire.into_bytes()
}

pub async fn output(
    wire: &[u8],
    chunk_size: usize,
) -> Vec<Result<Bytes, StreamError<rquest::Error>>> {
    let chunks: Vec<Result<Bytes, rquest::Error>> = wire
        .chunks(chunk_size)
        .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
        .collect();
    bedrock_converse::translate_openai_sse_to_bedrock_eventstream(
        stream::iter(chunks),
        "model".into(),
    )
    .collect()
    .await
}

pub fn decode(frame: &Bytes) -> (String, Value) {
    assert!(frame.len() >= 16);
    let number =
        |offset| u32::from_be_bytes(frame[offset..offset + 4].try_into().unwrap()) as usize;
    assert_eq!(number(0), frame.len());
    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    assert_eq!(number(8) as u32, crc.checksum(&frame[..8]));
    assert_eq!(
        number(frame.len() - 4) as u32,
        crc.checksum(&frame[..frame.len() - 4])
    );
    let end = 12 + number(4);
    assert!(end <= frame.len() - 4);
    let mut cursor = 12;
    let mut headers = BTreeMap::new();
    while cursor < end {
        let name_len = frame[cursor] as usize;
        cursor += 1;
        let name = std::str::from_utf8(&frame[cursor..cursor + name_len]).unwrap();
        cursor += name_len;
        assert_eq!(
            frame[cursor], 7,
            "all emitted headers must have string type"
        );
        cursor += 1;
        let len = u16::from_be_bytes(frame[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;
        let value = std::str::from_utf8(&frame[cursor..cursor + len]).unwrap();
        cursor += len;
        assert!(
            headers.insert(name, value).is_none(),
            "duplicate wire header"
        );
    }
    assert_eq!(cursor, end);
    assert_eq!(
        headers.len(),
        3,
        "native events require all three discriminators"
    );
    assert_eq!(headers[":message-type"], "event");
    assert_eq!(headers[":content-type"], "application/json");
    let kind = headers[":event-type"];
    let body: Value = serde_json::from_slice(&frame[end..frame.len() - 4]).unwrap();
    assert!(body.is_object());
    assert!(
        body.get(kind).is_none(),
        "payload must not have an SDK union wrapper"
    );
    (kind.into(), body)
}
