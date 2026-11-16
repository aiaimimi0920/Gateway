//! One buffered text turn consumes its process owner; cancellation therefore cannot leave it reusable.
use crate::{CopilotRuntime, RuntimeError};
use github_copilot_sdk::{session_events::AssistantMessageData, MessageOptions};
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub struct TextCompletion {
    /// SDK's final root assistant message, not concatenation of every message in the turn.
    pub text: String,
    /// Only the final assistant message's reported output, not total request usage or billable tokens.
    pub final_message_output_tokens: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("invalid Copilot single-turn text request")]
    InvalidRequest,
    #[error("Copilot text session could not start")]
    Session(#[source] RuntimeError),
    #[error("Copilot text execution failed")]
    Execute(#[source] github_copilot_sdk::Error),
    #[error("Copilot returned an unsupported or invalid text result")]
    InvalidResult,
    #[error("Copilot text runtime shutdown was not confirmed")]
    Shutdown(#[source] RuntimeError),
}

impl CopilotRuntime {
    /// No history, tools, media, client session IDs or arbitrary SDK options are accepted here.
    /// Caller owns startup and the overall deadline. Dropping this future drops the owned runtime,
    /// invoking SDK force_stop; it is not a graceful-stop receipt. Normal return awaits stop.
    pub async fn complete_text(
        mut self,
        token: String,
        model: String,
        prompt: String,
        wait_timeout: Duration,
    ) -> Result<TextCompletion, TextError> {
        let result = async {
            if token.is_empty()
                || token.len() > 16384
                || !token.bytes().all(|b| b.is_ascii_graphic())
                || model.trim().is_empty()
                || model.len() > 512
                || prompt.trim().is_empty()
                || prompt.len() > 64 * 1024
                || wait_timeout.is_zero()
                || wait_timeout > Duration::from_secs(120)
            {
                return Err(TextError::InvalidRequest);
            }
            let prepared = self
                .prepare_text_session(token.clone(), model)
                .map_err(TextError::Session)?;
            // Subscribe before creation; terminal correlation and timeout belong to the SDK waiter.
            let _events = prepared.subscribe();
            let session = prepared.start().await.map_err(TextError::Execute)?;
            let event = session
                .send_and_wait(MessageOptions::new(prompt).with_wait_timeout(wait_timeout))
                .await
                .map_err(TextError::Execute)?
                .ok_or(TextError::InvalidResult)?;
            let message: AssistantMessageData =
                event.typed_data().ok_or(TextError::InvalidResult)?;
            // send_and_wait returns the last root message. Do not misrepresent a multipart/tool
            // result as a complete single text response, or return credentials echoed by a peer.
            if message.content.len() > 1024 * 1024
                || message.content.contains(&token)
                || message.chunk_count.is_some_and(|count| count != 1)
                || message.chunk_index.is_some_and(|index| index != 0)
                || message
                    .tool_requests
                    .as_ref()
                    .is_some_and(|tools| !tools.is_empty())
                || message.server_tools.is_some()
            {
                return Err(TextError::InvalidResult);
            }
            let final_message_output_tokens = message
                .output_tokens
                .map(u64::try_from)
                .transpose()
                .map_err(|_| TextError::InvalidResult)?;
            Ok(TextCompletion {
                text: message.content,
                final_message_output_tokens,
            })
        }
        .await;
        // No replay, no detached cleanup task, and no successful result before owned shutdown.
        self.stop().await.map_err(TextError::Shutdown)?;
        result
    }
}
