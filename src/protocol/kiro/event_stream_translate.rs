//! Input polling, per-event backpressure and terminal resource ownership.
//! Events are admitted before either wire encoder mutates retained state.
use std::collections::HashMap;

use bytes::Bytes;
use futures::Stream;

use super::{
    push_anthropic_events, push_anthropic_finish, push_kiro_stream_error, push_openai_events,
    push_openai_finish, EventStreamParser, KiroEvent, TranslatorState,
};
use crate::{error::GatewayError, protocol::stream_error::ProtocolStreamError};

pub(super) fn translate_kiro_stream_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    state: TranslatorState,
    anthropic: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    translate(inner, state, anthropic, None)
}

pub(super) fn translate_kiro_key_stream<E: From<ProtocolStreamError> + Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    state: TranslatorState,
    anthropic: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    // Domain failure survives the Responses bridge; a plain Chat error object does not.
    translate(
        inner,
        state,
        anthropic,
        Some(|_| {
            ProtocolStreamError::invalid_data(
                "kiro_api_key_stream_upstream_error: Kiro CLI Key stream failed.",
            )
            .into()
        }),
    )
}

fn translate<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    state: TranslatorState,
    anthropic: bool,
    key_error: Option<fn(GatewayError) -> E>,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    futures::stream::unfold(
        (
            Box::pin(inner) as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
            if key_error.is_some() {
                EventStreamParser::for_api_key()
            } else {
                EventStreamParser::default()
            },
            state,
            anthropic,
            false,
            key_error,
        ),
        |(mut stream, mut parser, mut state, anthropic, mut done, key_error)| async move {
            use futures::StreamExt;
            loop {
                if done {
                    stream = Box::pin(futures::stream::empty());
                    release_inputs(&mut parser, &mut state);
                    if state.outputs.is_empty() {
                        return None;
                    }
                }
                // Drain this event's output before decoding or polling any more input.
                if let Some(output) = state.outputs.pop_front() {
                    return Some((
                        Ok(Bytes::from(output)),
                        (stream, parser, state, anthropic, done, key_error),
                    ));
                }

                let event = parser.next_event().and_then(|event| {
                    if let Some(event) = &event {
                        state.admit(event, anthropic)?;
                        let provider_failure = match event {
                            KiroEvent::Error { .. } => true,
                            KiroEvent::Exception { exception_type, .. } => {
                                exception_type != "ContentLengthExceededException"
                            }
                            _ => false,
                        };
                        if key_error.is_some() && provider_failure {
                            return Err(GatewayError::server_error("Kiro Key provider failure"));
                        }
                    }
                    Ok(event)
                });
                match event {
                    Ok(Some(event)) => {
                        let terminal_error = if anthropic {
                            push_anthropic_events(&mut state, event)
                        } else {
                            push_openai_events(&mut state, event)
                        };
                        if terminal_error {
                            done = true;
                        }
                        continue;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        if let Some(convert) = key_error {
                            release_inputs(&mut parser, &mut state);
                            return Some((
                                Err(convert(error)),
                                (
                                    Box::pin(futures::stream::empty()),
                                    parser,
                                    state,
                                    anthropic,
                                    true,
                                    key_error,
                                ),
                            ));
                        }
                        push_kiro_stream_error(&mut state, error, anthropic);
                        done = true;
                        continue;
                    }
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        if let Err(error) = parser.push(chunk) {
                            if let Some(convert) = key_error {
                                release_inputs(&mut parser, &mut state);
                                return Some((
                                    Err(convert(error)),
                                    (
                                        Box::pin(futures::stream::empty()),
                                        parser,
                                        state,
                                        anthropic,
                                        true,
                                        key_error,
                                    ),
                                ));
                            }
                            push_kiro_stream_error(&mut state, error, anthropic);
                            done = true;
                        }
                    }
                    Some(Err(error)) => {
                        stream = Box::pin(futures::stream::empty());
                        release_inputs(&mut parser, &mut state);
                        return Some((
                            Err(error),
                            (stream, parser, state, anthropic, true, key_error),
                        ));
                    }
                    None => {
                        if let Err(error) = parser.finish() {
                            if let Some(convert) = key_error {
                                release_inputs(&mut parser, &mut state);
                                return Some((
                                    Err(convert(error)),
                                    (
                                        Box::pin(futures::stream::empty()),
                                        parser,
                                        state,
                                        anthropic,
                                        true,
                                        key_error,
                                    ),
                                ));
                            }
                            push_kiro_stream_error(&mut state, error, anthropic);
                            done = true;
                            continue;
                        }
                        if anthropic {
                            push_anthropic_finish(&mut state);
                        } else {
                            push_openai_finish(&mut state);
                        }
                        done = true;
                    }
                }
            }
        },
    )
}

fn release_inputs(parser: &mut EventStreamParser, state: &mut TranslatorState) {
    // Preserve queued wire output, but release input/history before terminal delivery.
    parser.reset();
    state.response_id = String::new();
    state.anthropic_message_id = String::new();
    state.model = String::new();
    state.tool_name_map = HashMap::new();
    state.pending_tools = HashMap::new();
    state.retained_tool_bytes = 0;
}
