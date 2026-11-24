//! Project SAP Orchestration v2 SSE into Chat SSE before the shared terminal and usage guard.
use super::{orchestration::tools, stream_error};
use crate::protocol::{
    sse_parse::SseFrame,
    stream_decode::{BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES},
    stream_error::{ProtocolStreamError, StreamError},
    unique_json,
};
use bytes::Bytes;
use futures::{stream, Stream, StreamExt};
use serde_json::{json, Value};

type SapStreamError = StreamError<rquest::Error>;

pub(super) fn translate(
    response: rquest::Response,
    reply_model: String,
    allow_tools: bool,
    reasoning: bool,
) -> impl Stream<Item = Result<Bytes, SapStreamError>> + Send {
    let state = (
        Box::pin(response.bytes_stream()),
        BoundedSseDecoder::new_strict(MAX_TRANSLATED_SSE_FRAME_BYTES),
        Projection {
            reply_model,
            allow_tools,
            reasoning,
            ..Projection::default()
        },
    );
    stream::try_unfold(
        state,
        |(mut source, mut decoder, mut projection)| async move {
            loop {
                // Bookkeeping frames can be discarded without yielding bytes. Still yield to cancellation.
                tokio::task::consume_budget().await;
                match decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        if let Some(bytes) = projection.accept(frame)? {
                            return Ok(Some((bytes, (source, decoder, projection))));
                        }
                    }
                    DecodeStep::Error(_) => return Err(stream_error().into()),
                    DecodeStep::NeedInput => match source.next().await {
                        Some(Ok(bytes)) => decoder.push_chunk(bytes),
                        Some(Err(_)) => return Err(super::errors::interrupted().into()),
                        None => {
                            decoder.finish_strict().map_err(|_| stream_error())?;
                            return Ok(None);
                        }
                    },
                }
            }
        },
    )
}

#[derive(Default)]
struct Projection {
    reply_model: String,
    request_id: Option<String>,
    completion_id: Option<String>,
    done: bool,
    allow_tools: bool,
    saw_tools: bool,
    reasoning: bool,
}

impl Projection {
    fn accept(&mut self, frame: SseFrame) -> Result<Option<Bytes>, ProtocolStreamError> {
        if self.done
            || frame
                .event_name
                .as_deref()
                .is_some_and(|name| name != "message")
        {
            return Err(stream_error());
        }
        if frame.data == "[DONE]" {
            self.done = true;
            // The shared Chat guard releases DONE only after a real finish and clean EOF.
            return Ok(Some(Bytes::from_static(b"data: [DONE]\n\n")));
        }
        let envelope = unique_json::parse(frame.data.as_bytes()).map_err(|_| stream_error())?;
        if envelope.get("error").is_some_and(|value| !value.is_null()) {
            return Err(stream_error());
        }
        bind_id(&mut self.request_id, &envelope["request_id"])?;
        let Some(result) = envelope
            .get("final_result")
            .filter(|value| !value.is_null())
        else {
            // Module status, input echo, and LLM output before post-processing are never public.
            return Ok(None);
        };
        let choices = result["choices"].as_array().ok_or_else(stream_error)?;
        if self.completion_id.is_none() && is_placeholder(result, choices) {
            return Ok(None);
        }
        bind_id(&mut self.completion_id, &result["id"])?;
        if result["object"] != "chat.completion.chunk"
            || !result["model"]
                .as_str()
                .is_some_and(|model| !model.is_empty())
            || result["created"].as_u64().is_none()
        {
            return Err(stream_error());
        }
        let choices = match choices.as_slice() {
            [choice] => vec![project_choice(
                choice,
                self.allow_tools,
                self.reasoning,
                &mut self.saw_tools,
            )?],
            [] => Vec::new(), // An optional separate terminal usage frame has no choices.
            _ => return Err(stream_error()),
        };
        let mut chunk = json!({"id":result["id"],"object":result["object"],"model":self.reply_model,
            "created":result["created"],"choices":choices});
        if let Some(usage) = result.get("usage").filter(|value| !value.is_null()) {
            if usage["total_tokens"].as_u64().is_none() {
                return Err(stream_error());
            }
            // The shared guard validates counters, cached tokens, order, and uniqueness.
            chunk["usage"] = usage.clone();
        }
        Ok(Some(Bytes::from(format!("data: {chunk}\n\n"))))
    }
}

fn bind_id(expected: &mut Option<String>, value: &Value) -> Result<(), ProtocolStreamError> {
    let actual = value
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(stream_error)?;
    match expected {
        Some(expected) if expected != actual => Err(stream_error()),
        Some(_) => Ok(()),
        None => {
            *expected = Some(actual.to_owned());
            Ok(())
        }
    }
}

fn is_placeholder(result: &Value, choices: &[Value]) -> bool {
    result["id"] == ""
        && result["object"] == ""
        && result["model"] == ""
        && result["created"].as_u64() == Some(0)
        && result["usage"].is_null()
        && matches!(choices, [choice] if choice["index"].as_u64() == Some(0)
            && (choice["finish_reason"].is_null() || choice["finish_reason"] == "")
            && choice["delta"].as_object().is_some_and(|delta| delta.iter().all(|(key, value)|
                matches!(key.as_str(), "role" | "content") && (value.is_null() || value == ""))))
}

fn project_choice(
    choice: &Value,
    allow_tools: bool,
    reasoning: bool,
    saw_tools: &mut bool,
) -> Result<Value, ProtocolStreamError> {
    let source = choice["delta"].as_object().ok_or_else(stream_error)?;
    if choice["index"].as_u64() != Some(0)
        || source
            .get("function_call")
            .is_some_and(|value| !value.is_null())
    {
        return Err(stream_error());
    }
    let mut delta = serde_json::Map::new();
    for key in ["role", "content", "refusal"] {
        if let Some(value) = source
            .get(key)
            .filter(|value| !value.is_null() && *value != "")
        {
            if !value.is_string() {
                return Err(stream_error());
            }
            delta.insert(key.to_owned(), value.clone());
        }
    }
    if reasoning {
        if let Some(blocks) = source.get("reasoning_content").filter(|v| !v.is_null()) {
            if !super::orchestration::reasoning::valid_blocks(blocks) {
                return Err(stream_error());
            }
            delta.insert("reasoning_content".to_owned(), blocks.clone());
        }
    }
    if let Some(calls) = source.get("tool_calls").filter(|value| !value.is_null()) {
        let chunks = calls.as_array().ok_or_else(stream_error)?;
        if !chunks.is_empty() {
            if !allow_tools || !tools::valid_chunks(chunks) {
                return Err(stream_error());
            }
            // The shared Chat guard owns per-index fragment identity and completion checks.
            delta.insert("tool_calls".into(), calls.clone());
            *saw_tools = true;
        }
    }
    let finish = &choice["finish_reason"];
    let finish = if finish == "" { &Value::Null } else { finish };
    if !finish.is_null() && (finish == "function_call" || (finish == "tool_calls") != *saw_tools) {
        return Err(stream_error());
    }
    Ok(json!({"index":0,"delta":delta,"finish_reason":finish}))
}
