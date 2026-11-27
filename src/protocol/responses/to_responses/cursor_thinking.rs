//! Explicit Cursor-only buffered metadata; arbitrary Chat extension fields never opt in.
use super::*;
use crate::protocol::cursor_remote::thinking::PublicThinking;

pub(crate) fn translate_cursor_sse_to_responses<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    thinking: Option<PublicThinking>,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    translate_openai_sse_to_responses_with_limit_and_error(
        inner,
        model,
        MAX_TRANSLATED_SSE_FRAME_BYTES,
        thinking,
    )
}

impl OpenAiToResponsesState {
    pub(super) fn emit_cursor_thinking(&mut self) {
        if let Some(thinking) = &self.cursor_thinking {
            let events = thinking.events(self.next_output_index);
            self.allocate_output_index();
            for mut event in events {
                event["sequence_number"] = self.next_sequence_number().into();
                let kind = event["type"].as_str().expect("typed thinking event");
                self.outputs
                    .push_back(format_sse_event(Some(kind), &event.to_string()).into_bytes());
            }
        }
    }
}
