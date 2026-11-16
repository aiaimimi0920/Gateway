//! Native Messages counters remain authoritative internally; public OpenAI inputs include cache.
use crate::protocol::{
    canonical::TokenUsage,
    stream_decode::{BoundedSseDecoder, DecodeStep},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};
pub(super) fn inclusive(mut usage: TokenUsage) -> Option<TokenUsage> {
    usage.prompt_tokens = usage
        .prompt_tokens
        .checked_add(usage.cache_read_input_tokens.unwrap_or(0))?
        .checked_add(usage.cache_creation_input_tokens.unwrap_or(0))?;
    usage.total_tokens = usage.prompt_tokens.checked_add(usage.completion_tokens)?;
    Some(usage)
}
/// Called after the native usage tap. These are complete frames produced by our Messages owner.
pub(crate) fn openai_stream<S, E>(input: S) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    input.map(|chunk| project(chunk?).map_err(|_| super::stream_error().into()))
}
fn project(bytes: Bytes) -> Result<Bytes, ()> {
    let mut decoder = BoundedSseDecoder::new_strict(64 * 1024 * 1024);
    decoder.push_chunk(bytes);
    let mut out = String::new();
    loop {
        let frame = match decoder.decode_next() {
            DecodeStep::Frame(frame) => frame,
            DecodeStep::NeedInput => break,
            DecodeStep::Error(_) => return Err(()),
        };
        if frame.data == "[DONE]" {
            out.push_str("data: [DONE]\n\n");
            continue;
        }
        let mut value: Value = serde_json::from_str(&frame.data).map_err(|_| ())?;
        if let Some(usage) = value.get_mut("usage").filter(|v| !v.is_null()) {
            let prompt = usage["prompt_tokens"]
                .as_u64()
                .ok_or(())?
                .checked_add(
                    usage["cache_read_input_tokens"]
                        .as_u64()
                        .or_else(|| {
                            usage
                                .pointer("/prompt_tokens_details/cached_tokens")
                                .and_then(Value::as_u64)
                        })
                        .unwrap_or(0),
                )
                .and_then(|v| {
                    v.checked_add(usage["cache_creation_input_tokens"].as_u64().unwrap_or(0))
                })
                .ok_or(())?;
            let completion = usage["completion_tokens"].as_u64().ok_or(())?;
            usage["prompt_tokens"] = prompt.into();
            usage["total_tokens"] = prompt.checked_add(completion).ok_or(())?.into();
            if let Some(read) = usage["cache_read_input_tokens"].as_u64().or_else(|| {
                usage
                    .pointer("/prompt_tokens_details/cached_tokens")
                    .and_then(Value::as_u64)
            }) {
                usage["prompt_tokens_details"] = json!({"cached_tokens":read});
            }
        }
        out.push_str(&format!("data: {value}\n\n"));
    }
    decoder.finish_strict().map_err(|_| ())?;
    Ok(Bytes::from(out))
}
