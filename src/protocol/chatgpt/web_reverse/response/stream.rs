//! Lazy, bounded translation with explicit completion and typed protocol failures.
use super::{is_non_text_control_frame, process_event, text_delta, ConversationState};
use crate::error::GatewayError;
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn translate_chatgpt_web_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, StreamError<rquest::Error>>> + Send + 'static {
    let state = State {
        decoder: BoundedSseDecoder::new(MAX_TRANSLATED_SSE_FRAME_BYTES),
        conversation: ConversationState::default(),
        model,
        response_id: format!("chatcmpl_chatgptweb_{}", uuid::Uuid::new_v4().simple()),
        created: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        ending: false,
        ended: false,
    };
    futures::stream::unfold(
        (Some(Box::pin(inner)), state),
        |(mut source, mut state)| async move {
            loop {
                if state.ended {
                    return None;
                }
                if state.ending {
                    state.ended = true;
                    return Some((Ok(Bytes::from_static(b"data: [DONE]\n\n")), (None, state)));
                }
                match state.decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        let data = frame.data.trim();
                        if data == "[DONE]" {
                            // Release the HTTP source without draining it after a terminal event.
                            source = None;
                            let result = state.finish(true).map_err(StreamError::Protocol);
                            return Some((result, (source, state)));
                        }
                        match state.process_frame(data) {
                            Ok(Some(bytes)) => return Some((Ok(bytes), (source, state))),
                            Ok(None) => {}
                            Err(error) => {
                                state.ended = true;
                                state.release_buffers();
                                return Some((Err(StreamError::Protocol(error)), (None, state)));
                            }
                        }
                    }
                    DecodeStep::Error(error) => {
                        state.ended = true;
                        state.release_buffers();
                        return Some((Err(StreamError::Protocol(error)), (None, state)));
                    }
                    DecodeStep::NeedInput => {
                        match source.as_mut().expect("live source").next().await {
                            Some(Ok(chunk)) => state.decoder.push_chunk(chunk),
                            Some(Err(error)) => {
                                state.ended = true;
                                state.release_buffers();
                                return Some((Err(StreamError::Transport(error)), (None, state)));
                            }
                            None => {
                                // Do not promote an unterminated SSE frame to a completion signal.
                                source = None;
                                let result = state.finish(false).map_err(StreamError::Protocol);
                                return Some((result, (source, state)));
                            }
                        }
                    }
                }
            }
        },
    )
}

struct State {
    decoder: BoundedSseDecoder,
    conversation: ConversationState,
    model: String,
    response_id: String,
    created: i64,
    ending: bool,
    ended: bool,
}

fn protocol_error(error: GatewayError) -> ProtocolStreamError {
    // Only our fixed completion errors cross this boundary, never upstream JSON/HTML.
    ProtocolStreamError::invalid_data(format!(
        "{}: {}",
        error.code.as_deref().unwrap_or("chatgpt_web_stream_failed"),
        error.message
    ))
}

impl State {
    fn process_frame(&mut self, data: &str) -> Result<Option<Bytes>, ProtocolStreamError> {
        if data.is_empty() || data == "\"v1\"" || data == "v1" {
            return Ok(None);
        }
        let before = self.conversation.text.clone();
        self.conversation.observe_status(data);
        match serde_json::from_str::<Value>(data) {
            Ok(value) => process_event(&value, &mut self.conversation),
            Err(_) if !is_non_text_control_frame(data) => self.conversation.append_text(data),
            Err(_) => {}
        }
        if let Some(error) = self.conversation.failure() {
            return Err(protocol_error(error));
        }
        if self.conversation.text.len() > MAX_TRANSLATED_SSE_FRAME_BYTES {
            return Err(ProtocolStreamError::invalid_data(
                "chatgpt_web_stream_text_limit: assistant text exceeded the bounded response limit",
            ));
        }
        Ok(text_delta(&before, &self.conversation.text)
            .map(|delta| self.content_chunk(&delta, None)))
    }

    fn finish(&mut self, saw_done: bool) -> Result<Bytes, ProtocolStreamError> {
        let result = match self.conversation.finish_reason(saw_done) {
            Ok(reason) => {
                self.ending = true;
                Ok(self.content_chunk("", Some(reason)))
            }
            Err(error) => {
                self.ended = true;
                Err(protocol_error(error))
            }
        };
        self.release_buffers();
        result
    }

    fn release_buffers(&mut self) {
        self.decoder.clear();
        self.conversation.text = String::new();
    }

    fn content_chunk(&self, content: &str, finish_reason: Option<&str>) -> Bytes {
        let delta = if content.is_empty() {
            json!({})
        } else {
            json!({"content":content})
        };
        let chunk = json!({
            "id":self.response_id,"object":"chat.completion.chunk","created":self.created,
            "model":self.model,"choices":[{"index":0,"delta":delta,"finish_reason":finish_reason}]
        });
        Bytes::from(format!("data: {chunk}\n\n"))
    }
}
