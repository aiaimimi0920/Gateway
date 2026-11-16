//! Build the terminal Responses payload after the incremental owner's completion checks.
use super::OpenAiToResponsesState;
use crate::protocol::response_sources;
use serde_json::{json, Value};

impl OpenAiToResponsesState {
    pub(super) fn build_completed_response(&self, finish_status: Option<&str>) -> Value {
        let mut output = Vec::<(usize, Value)>::new();
        if let Some(message) = self.message.as_ref() {
            output.push((
                message.output_index,
                json!({
                    "id": message.item_id,
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": message.text}],
                }),
            ));
        }
        for tool_call in self.tool_calls.values() {
            output.push((tool_call.output_index, tool_call.item("completed")));
        }
        output.sort_by_key(|(output_index, _)| *output_index);
        let output = output.into_iter().map(|(_, item)| item).collect::<Vec<_>>();
        let status = finish_status.unwrap_or("completed");
        let mut response = json!({
            "id": self.response_id,
            "object": "response",
            "model": self.model,
            "output": output,
            "status": status,
        });
        if let Some(usage) = self.latest_usage.as_ref() {
            response["usage"] = json!({
                "input_tokens": usage.prompt_tokens,
                "output_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
                "input_tokens_details": {"cached_tokens": usage.cache_read_input_tokens.unwrap_or(0)},
            });
        }
        response_sources::project(&mut response, self.sources.as_deref().unwrap_or_default());
        crate::protocol::vertex_turn::project(&mut response, self.vertex_turn.0.as_ref());
        self.bedrock.project(&mut response);
        response
    }
}
