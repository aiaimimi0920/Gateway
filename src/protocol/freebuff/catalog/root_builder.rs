//! Default CLI root request assembly for a TRUSTED host, not a tool executor or public admission.
//! The host owns bare history/context and actual compaction; this prepares only the consumed snapshot.
use super::{root_context::RootRequest, BusinessError, BusinessRequests};
use serde_json::{json, Value};
mod cli;
mod guidance;
pub(super) mod history;
mod merge;
mod prompt;
mod reminders;

// This is the default fixed CLI surface, not a claim of server-required tools or permission to run them.
const TOOLS: [&str; 15] = [
    "read_files",
    "str_replace",
    "write_file",
    "run_terminal_command",
    "code_search",
    "glob",
    "list_directory",
    "write_todos",
    "web_search",
    "read_url",
    "ask_user",
    "suggest_followups",
    "gravity_index",
    "render_ui",
    "skill",
];

/// Already-rendered host context; no filesystem discovery, shell access or recursive placeholders.
pub(crate) struct RootHostContext<'a> {
    pub(crate) current_date: &'a str,
    pub(crate) knowledge_files: &'a str,
    pub(crate) git_changes: &'a str,
    pub(crate) system_info: &'a str,
}
/// A fresh native history snapshot WITHOUT generated instruction/step reminders.
/// Ownership is consumed only for this request; the host retains its canonical history separately.
pub(crate) struct RootStep<'a> {
    pub(crate) context: RootHostContext<'a>,
    pub(crate) messages: Vec<Value>,
    pub(crate) tools: Vec<Value>,
    pub(crate) stream: bool,
}
impl BusinessRequests<'_> {
    pub(crate) fn build_root_step(
        &self,
        step: RootStep<'_>,
        limit: usize,
    ) -> Result<(Value, RootRequest), BusinessError> {
        let context_bytes = [
            step.context.current_date,
            step.context.knowledge_files,
            step.context.git_changes,
            step.context.system_info,
        ]
        .iter()
        .try_fold(0usize, |total, value| total.checked_add(value.len()));
        if context_bytes.is_none_or(|size| size > 1024 * 1024)
            || step.context.current_date.trim().is_empty()
            || step.context.current_date.chars().any(char::is_control)
            || step.tools.len() != TOOLS.len()
            || TOOLS.iter().any(|name| {
                step.tools
                    .iter()
                    .filter(|tool| {
                        tool.get("type").and_then(Value::as_str) == Some("function")
                            && tool.pointer("/function/name").and_then(Value::as_str) == Some(*name)
                            && tool
                                .pointer("/function/parameters")
                                .is_some_and(Value::is_object)
                    })
                    .count()
                    != 1
            })
        {
            return Err(BusinessError::InvalidInput);
        }
        let history = history::prepare(step.messages, limit)?;
        let model = self.root_model_input();
        // Opaque catalog handles do not match the reference's named-model exceptions.
        // This is a runtime policy budget, not a verified provider/model context ceiling.
        let context_limit = model
            .compaction
            .and_then(|policy| policy.max_context_length)
            .unwrap_or(400_000);
        let mut history_indices = Vec::with_capacity(history.len());
        let mut messages = Vec::with_capacity(2 + history.len() * 2);
        messages.push(json!({"role":"system","content":prompt::system(&model,&step.context)}));
        // Rebuild only from the host's bare snapshot. No generated reminder is written back to it.
        for message in history {
            let user = message["role"] == "user";
            history_indices.push(messages.len());
            messages.push(message);
            if user && model.turn_ending_note {
                messages.push(json!({"role":"user","content":reminders::TURN_ENDING_NOTE}));
            }
        }
        if model.batching {
            messages.push(json!({"role":"user","content":reminders::STEP}));
        }
        // Catalog handles do NOT start with anthropic/: compiled identity must not add a provider pin.
        // Move native history/tools without flattening reasoning, tool calls, images or host extensions.
        let mut body = serde_json::Map::new();
        body.insert("model".into(), Value::String(model.model.into()));
        body.insert("messages".into(), Value::Array(messages));
        body.insert("tools".into(), Value::Array(step.tools));
        body.insert("stream".into(), Value::Bool(step.stream));
        body.insert("provider".into(), json!({"data_collection":"deny"}));
        let body = Value::Object(body);
        let context = RootRequest::new(&body, &history_indices, context_limit)?;
        Ok((body, context))
    }
}
#[cfg(test)]
pub(super) mod tests;
