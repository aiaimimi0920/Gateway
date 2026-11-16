//! Content-Type selects JSON or SSE. Buffered aggregation is bounded and retains exact function strings.
use super::response_error;
use crate::{
    error::GatewayError,
    protocol::{
        canonical::CanonicalRelayResponse,
        openai,
        stream_decode::{BoundedSseDecoder, DecodeStep},
        stream_error::StreamError,
    },
};
use futures::StreamExt;
use serde_json::{json, Value};
use std::collections::BTreeMap;
const LIMIT: usize = 16 * 1024 * 1024;
pub(super) fn is_sse(response: &rquest::Response) -> Result<bool, GatewayError> {
    match response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::trim)
    {
        Some(v) if v.eq_ignore_ascii_case("text/event-stream") => Ok(true),
        Some(v) if v.eq_ignore_ascii_case("application/json") => Ok(false),
        _ => Err(response_error()),
    }
}
pub(super) async fn read(
    response: rquest::Response,
) -> Result<CanonicalRelayResponse, GatewayError> {
    read_inner(response, LIMIT, false).await
}
pub(super) async fn read_probe(
    response: rquest::Response,
) -> Result<CanonicalRelayResponse, GatewayError> {
    read_inner(response, super::probe_admission::LIMIT, true).await
}
async fn read_inner(
    response: rquest::Response,
    limit: usize,
    probe: bool,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let sse = is_sse(&response)?;
    let body =
        crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(),
            "LobsterAI Chat",
            limit,
            None,
        )
        .await
        .map_err(|_| response_error())?;
    if probe {
        super::probe_admission::chat(&body, sse)?;
    }
    let value = if sse {
        aggregate(body.into()).await?
    } else {
        crate::protocol::unique_json::parse(&body).map_err(|_| response_error())?
    };
    openai::chat_admission::validate(&value).map_err(|_| response_error())?;
    openai::unpack_native_chat_response(&value).map_err(|_| response_error())
}
pub(super) async fn aggregate(body: bytes::Bytes) -> Result<Value, GatewayError> {
    aggregate_as(body, false).await
}
pub(super) async fn aggregate_messages(body: bytes::Bytes) -> Result<Value, GatewayError> {
    aggregate_as(body, true).await
}
async fn aggregate_as(body: bytes::Bytes, messages: bool) -> Result<Value, GatewayError> {
    let input = futures::stream::once(async { Ok::<_, StreamError<rquest::Error>>(body) });
    let mut stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<bytes::Bytes, StreamError<rquest::Error>>> + Send>,
    > = if messages {
        Box::pin(super::stream::from_messages(input, None))
    } else {
        Box::pin(super::stream::validated(input))
    };
    let mut decoder = BoundedSseDecoder::new_strict(LIMIT);
    let mut text = String::new();
    let mut tools: BTreeMap<u64, Value> = BTreeMap::new();
    let mut finish = None;
    let mut usage = Value::Null;
    let mut size = 0usize;
    while let Some(bytes) = stream.next().await {
        let bytes = bytes.map_err(|_| response_error())?;
        size = size
            .checked_add(bytes.len())
            .filter(|n| *n <= LIMIT)
            .ok_or_else(response_error)?;
        decoder.push_chunk(bytes);
        loop {
            let frame = match decoder.decode_next() {
                DecodeStep::Frame(frame) => frame,
                DecodeStep::NeedInput => break,
                DecodeStep::Error(_) => return Err(response_error()),
            };
            if frame.data == "[DONE]" {
                continue;
            }
            let value: Value = serde_json::from_str(&frame.data).map_err(|_| response_error())?;
            if let Some(value) = value.get("usage").filter(|v| !v.is_null()) {
                usage = value.clone();
            }
            for choice in value["choices"].as_array().ok_or_else(response_error)? {
                if choice["index"] != 0 {
                    return Err(response_error());
                }
                let delta = &choice["delta"];
                if let Some(value) = delta["content"].as_str() {
                    text.push_str(value);
                }
                if delta.get("function_call").is_some_and(|v| !v.is_null()) {
                    return Err(response_error());
                }
                if let Some(calls) = delta["tool_calls"].as_array() {
                    for call in calls {
                        let index = call["index"].as_u64().ok_or_else(response_error)?;
                        let tool = tools.entry(index).or_insert_with(|| json!({"id":"","type":"function","function":{"name":"","arguments":""}}));
                        append(&mut tool["id"], &call["id"])?;
                        append(&mut tool["function"]["name"], &call["function"]["name"])?;
                        append(
                            &mut tool["function"]["arguments"],
                            &call["function"]["arguments"],
                        )?;
                    }
                }
                if let Some(value) = choice["finish_reason"].as_str() {
                    finish = Some(value.to_owned());
                }
            }
        }
    }
    decoder.finish_strict().map_err(|_| response_error())?;
    let mut message = json!({"role":"assistant","content":text});
    if !tools.is_empty() {
        message["tool_calls"] = tools.into_values().collect();
    }
    Ok(
        json!({"choices":[{"index":0,"message":message,"finish_reason":finish.ok_or_else(response_error)?}],"usage":usage}),
    )
}
fn append(target: &mut Value, source: &Value) -> Result<(), GatewayError> {
    if source.is_null() {
        return Ok(());
    }
    let Value::String(target) = target else {
        return Err(response_error());
    };
    target.push_str(source.as_str().ok_or_else(response_error)?);
    Ok(())
}
