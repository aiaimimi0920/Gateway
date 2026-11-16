//! Preserve replaceable live observations without mutating standard append-only Responses text.
use super::*;
use crate::protocol::perplexity_web::public_snapshot;
impl OpenAiToResponsesState {
    pub(super) fn handle_perplexity_snapshot(&mut self, chunk: &Value) -> bool {
        match self.perplexity.observe(chunk) {
            Ok(None) => return false,
            Ok(Some(snapshot))
                if self.message.is_none()
                    && self.sources.is_none()
                    && self.tool_calls.is_empty()
                    && self.latest_usage.is_none()
                    && self.pending_finish_status.is_none()
                    && !self.final_emitted
                    && !self.native_passthrough
                    && !self.bedrock.active() =>
            {
                self.ensure_response_started();
                let sequence = self.next_sequence_number();
                let event = serde_json::json!({"type":public_snapshot::EVENT,
                    "sequence_number":sequence,"response_id":self.response_id,"snapshot":snapshot});
                self.outputs.push_back(
                    format_sse_event(Some(public_snapshot::EVENT), &event.to_string()).into_bytes(),
                );
            }
            Ok(Some(_)) => {
                self.failure = Some(ProtocolStreamError::invalid_data(
                    "Perplexity snapshot after standard answer content",
                ))
            }
            Err(error) => self.failure = Some(error),
        }
        true
    }
}

#[cfg(test)]
mod tests;
