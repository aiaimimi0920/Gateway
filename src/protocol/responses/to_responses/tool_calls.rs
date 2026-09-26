//! Tool-call identity, argument accumulation and closing events for Responses SSE.

use serde_json::Value;

use super::super::to_responses_events::{
    build_response_function_call_arguments_delta, build_response_function_call_arguments_done,
    build_response_function_call_item_added, build_response_function_call_item_done,
};
use super::OpenAiToResponsesState;

#[derive(Debug, Clone)]
pub(super) struct PendingResponseToolCall {
    pub(super) item_id: String,
    pub(super) call_id: String,
    pub(super) output_index: usize,
    pub(super) name: Option<String>,
    pub(super) arguments: String,
    pub(super) pending_argument_deltas: Vec<String>,
    pub(super) announced: bool,
    pub(super) done: bool,
}

impl OpenAiToResponsesState {
    pub(super) fn handle_tool_call_delta(&mut self, tool_call: &Value, fallback_index: usize) {
        let openai_index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(fallback_index);

        let mut add_event = None;
        let mut delta_event = None;
        let sequence_number = &mut self.sequence_number;
        let next_output_index = &mut self.next_output_index;

        {
            let entry = self.tool_calls.entry(openai_index).or_insert_with(|| {
                let output_index = *next_output_index;
                *next_output_index += 1;
                let synthetic_call_id = format!("call_{}", uuid::Uuid::new_v4().as_simple());
                PendingResponseToolCall {
                    item_id: format!("fc_{synthetic_call_id}"),
                    call_id: synthetic_call_id,
                    output_index,
                    name: None,
                    arguments: String::new(),
                    pending_argument_deltas: Vec::new(),
                    announced: false,
                    done: false,
                }
            });

            if !entry.announced {
                if let Some(call_id) = tool_call.get("id").and_then(|value| value.as_str()) {
                    if !call_id.trim().is_empty() {
                        entry.call_id = call_id.to_string();
                        entry.item_id = format!("fc_{call_id}");
                    }
                }
            }

            if let Some(name) = tool_call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(|value| value.as_str())
            {
                if !name.trim().is_empty() {
                    entry.name = Some(name.to_string());
                }
            }

            if !entry.announced {
                if let Some(arguments) = tool_call
                    .get("function")
                    .and_then(|function| function.get("arguments"))
                    .and_then(|value| value.as_str())
                {
                    if !arguments.is_empty() {
                        entry.arguments.push_str(arguments);
                        entry.pending_argument_deltas.push(arguments.to_string());
                    }
                }

                if let Some(name) = entry.name.clone() {
                    *sequence_number += 1;
                    add_event = Some(build_response_function_call_item_added(
                        *sequence_number,
                        entry.output_index,
                        &entry.item_id,
                        &entry.call_id,
                        &name,
                    ));
                    if !entry.pending_argument_deltas.is_empty() {
                        let mut batched = Vec::new();
                        for pending_delta in entry.pending_argument_deltas.drain(..) {
                            *sequence_number += 1;
                            batched.push(build_response_function_call_arguments_delta(
                                *sequence_number,
                                entry.output_index,
                                &entry.item_id,
                                &pending_delta,
                            ));
                        }
                        delta_event = Some(batched);
                    }
                    entry.announced = true;
                }
            } else if let Some(arguments) = tool_call
                .get("function")
                .and_then(|function| function.get("arguments"))
                .and_then(|value| value.as_str())
            {
                if !arguments.is_empty() {
                    entry.arguments.push_str(arguments);
                    *sequence_number += 1;
                    delta_event = Some(vec![build_response_function_call_arguments_delta(
                        *sequence_number,
                        entry.output_index,
                        &entry.item_id,
                        arguments,
                    )]);
                }
            }
        }

        if let Some(event) = add_event {
            self.outputs.push_back(event.into_bytes());
        }
        if let Some(events) = delta_event {
            for event in events {
                self.outputs.push_back(event.into_bytes());
            }
        }
    }

    pub(super) fn close_tool_item(&mut self, openai_index: usize) {
        let Some(snapshot) = self
            .tool_calls
            .get(&openai_index)
            .filter(|entry| !entry.done)
            .map(|entry| {
                (
                    entry.announced,
                    entry.output_index,
                    entry.item_id.clone(),
                    entry.call_id.clone(),
                    entry
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("tool_{openai_index}")),
                    crate::protocol::accio::normalize_tool_args(&entry.arguments),
                )
            })
        else {
            return;
        };

        let (announced, output_index, item_id, call_id, name, arguments) = snapshot;

        let add_event = if !announced {
            let add_seq = self.next_sequence_number();
            Some(build_response_function_call_item_added(
                add_seq,
                output_index,
                &item_id,
                &call_id,
                &name,
            ))
        } else {
            None
        };

        let args_done_seq = self.next_sequence_number();
        let args_done_event = build_response_function_call_arguments_done(
            args_done_seq,
            output_index,
            &item_id,
            &arguments,
        );

        let item_done_seq = self.next_sequence_number();
        let item_done_event = build_response_function_call_item_done(
            item_done_seq,
            output_index,
            &item_id,
            &call_id,
            &name,
            &arguments,
        );

        if let Some(entry) = self.tool_calls.get_mut(&openai_index) {
            entry.name = Some(name);
            entry.arguments = arguments;
            entry.pending_argument_deltas.clear();
            entry.announced = true;
            entry.done = true;
        }

        if let Some(event) = add_event {
            self.outputs.push_back(event.into_bytes());
        }
        self.outputs.push_back(args_done_event.into_bytes());
        self.outputs.push_back(item_done_event.into_bytes());
    }
}
