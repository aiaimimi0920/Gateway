//! Input polling, per-event backpressure and terminal resource ownership.
//! Events are admitted before either wire encoder mutates retained state.
use std::collections::HashMap;

use bytes::Bytes;
use futures::Stream;

use super::{
    push_anthropic_events, push_anthropic_finish, push_kiro_stream_error, push_openai_events,
    push_openai_finish, EventStreamParser, TranslatorState,
};

pub(super) fn translate_kiro_stream_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    state: TranslatorState,
    anthropic: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    futures::stream::unfold(
        (
            Box::pin(inner) as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
            EventStreamParser::default(),
            state,
            anthropic,
            false,
        ),
        |(mut stream, mut parser, mut state, anthropic, mut done)| async move {
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
                        (stream, parser, state, anthropic, done),
                    ));
                }

                match parser.next_event() {
                    Ok(Some(event)) => {
                        if let Err(error) = state.admit(&event, anthropic) {
                            push_kiro_stream_error(&mut state, error, anthropic);
                            done = true;
                            continue;
                        }
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
                        push_kiro_stream_error(&mut state, error, anthropic);
                        done = true;
                        continue;
                    }
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        if let Err(error) = parser.push(chunk) {
                            push_kiro_stream_error(&mut state, error, anthropic);
                            done = true;
                        }
                    }
                    Some(Err(error)) => {
                        stream = Box::pin(futures::stream::empty());
                        release_inputs(&mut parser, &mut state);
                        return Some((Err(error), (stream, parser, state, anthropic, true)));
                    }
                    None => {
                        if let Err(error) = parser.finish() {
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
