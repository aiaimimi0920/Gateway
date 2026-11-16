// Adapted from CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d.
// Copyright 2025 Codebuff. Apache-2.0; see LICENSE, NOTICE and README.md here.
// Modifications: Rust rendering, trusted explicit host context, one-pass substitution.
pub(super) const TURN_ENDING_NOTE: &str = r###"<system>Do not reply with a status line or an announcement of your next step. If the message above needs work, your response must call the tools that do it; a response without a tool call ends your turn and nothing runs after it. If it needs no work, or asks only for an answer, explanation or plan, give it here in full.</system>"###;
pub(super) const STEP: &str = r###"<system_reminder>Work in batches: put every tool call you can already make into this response (all the searches you can think of, every file worth reading, all the edits you know with their checks in one command). Once every part of the request is done and checked, give the final message instead.</system_reminder>"###;
