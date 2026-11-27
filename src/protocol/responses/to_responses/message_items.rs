//! Message item lifecycle, sequence numbering, and terminal event emission for Responses SSE.
use super::super::to_responses_events::{
    build_response_content_part_added, build_response_content_part_done,
    build_response_created_event, build_response_in_progress_event,
    build_response_message_item_added, build_response_message_item_done,
    build_response_output_text_delta, build_response_output_text_done,
    build_response_terminal_event,
};
use super::OpenAiToResponsesState;

#[derive(Debug, Clone)]
pub(super) struct PendingResponseMessage {
    pub(super) item_id: String,
    pub(super) output_index: usize,
    pub(super) text: String,
    pub(super) done: bool,
}

impl OpenAiToResponsesState {
    pub(super) fn ensure_response_started(&mut self) {
        if self.response_started {
            return;
        }
        self.response_started = true;
        let created_seq = self.next_sequence_number();
        let created = build_response_created_event(
            created_seq,
            &self.response_id,
            &self.model,
            self.created_at,
        );
        self.outputs.push_back(created.into_bytes());

        let progress_seq = self.next_sequence_number();
        let in_progress = build_response_in_progress_event(
            progress_seq,
            &self.response_id,
            &self.model,
            self.created_at,
        );
        self.outputs.push_back(in_progress.into_bytes());
        self.emit_cursor_thinking();
    }

    pub(super) fn handle_text_delta(&mut self, text: &str) {
        let (output_index, item_id) = self.ensure_message_item();
        if let Some(message) = self.message.as_mut() {
            message.text.push_str(text);
        }
        let sequence_number = self.next_sequence_number();
        let delta = build_response_output_text_delta(sequence_number, output_index, &item_id, text);
        self.outputs.push_back(delta.into_bytes());
    }

    pub(super) fn ensure_message_item(&mut self) -> (usize, String) {
        if let Some(message) = &self.message {
            return (message.output_index, message.item_id.clone());
        }

        let output_index = self.allocate_output_index();
        let item_id = format!("msg_{}", uuid::Uuid::new_v4().as_simple());
        let item_added_seq = self.next_sequence_number();
        let item_added = build_response_message_item_added(item_added_seq, output_index, &item_id);
        self.outputs.push_back(item_added.into_bytes());

        let part_added_seq = self.next_sequence_number();
        let part_added = build_response_content_part_added(part_added_seq, output_index, &item_id);
        self.outputs.push_back(part_added.into_bytes());
        self.message = Some(PendingResponseMessage {
            item_id: item_id.clone(),
            output_index,
            text: String::new(),
            done: false,
        });
        (output_index, item_id)
    }

    pub(super) fn emit_final_if_needed(&mut self, fallback_status: Option<&str>) {
        if self.final_emitted || self.native_passthrough || self.bedrock.hold_terminal() {
            return;
        }

        self.ensure_response_started();
        self.close_open_items();

        let finish_status = self
            .pending_finish_status
            .clone()
            .or_else(|| fallback_status.map(str::to_string));
        let response = self.build_completed_response(finish_status.as_deref());

        let sequence_number = self.next_sequence_number();
        let completed = build_response_terminal_event(sequence_number, response);
        self.outputs.push_back(completed.into_bytes());
        self.final_emitted = true;
    }

    pub(super) fn close_open_items(&mut self) {
        let mut close_order = Vec::new();
        if let Some(message) = self.message.as_ref() {
            if !message.done {
                close_order.push((message.output_index, None));
            }
        }
        for (openai_index, tool_call) in &self.tool_calls {
            if !tool_call.done {
                close_order.push((tool_call.output_index, Some(*openai_index)));
            }
        }
        close_order.sort_by_key(|(output_index, _)| *output_index);

        for (_, tool_index) in close_order {
            match tool_index {
                Some(openai_index) => self.close_tool_item(openai_index),
                None => self.close_message_item(),
            }
        }
    }

    pub(super) fn close_message_item(&mut self) {
        let Some((output_index, item_id, text)) = self
            .message
            .as_ref()
            .filter(|message| !message.done)
            .map(|message| {
                (
                    message.output_index,
                    message.item_id.clone(),
                    message.text.clone(),
                )
            })
        else {
            return;
        };

        let text_done_seq = self.next_sequence_number();
        let text_done =
            build_response_output_text_done(text_done_seq, output_index, &item_id, &text);
        self.outputs.push_back(text_done.into_bytes());

        let part_done_seq = self.next_sequence_number();
        let part_done =
            build_response_content_part_done(part_done_seq, output_index, &item_id, &text);
        self.outputs.push_back(part_done.into_bytes());

        let item_done_seq = self.next_sequence_number();
        let item_done =
            build_response_message_item_done(item_done_seq, output_index, &item_id, &text);
        self.outputs.push_back(item_done.into_bytes());

        if let Some(message) = self.message.as_mut() {
            message.done = true;
        }
    }

    pub(super) fn allocate_output_index(&mut self) -> usize {
        let index = self.next_output_index;
        self.next_output_index += 1;
        index
    }

    pub(super) fn next_sequence_number(&mut self) -> u64 {
        self.sequence_number = self.sequence_number.saturating_add(1);
        self.sequence_number
    }
}
