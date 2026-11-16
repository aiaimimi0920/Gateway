//! M Plan generation plugs into the existing Messages audit, timeout and cancellation owner.
use super::console_model_probe::ModelProbeResult;
use crate::{routing::config::CredentialProbeTarget, state::AppState};
use serde_json::{json, Value};

pub(super) async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> ModelProbeResult {
    match crate::protocol::minimax::probe::execute(
        &state.upstream_client, &target.payload, model, prompt,
    ).await {
        Ok((status, response)) => ModelProbeResult {
            status: Some(status), reply: Ok(response.text),
            usage: response.usage.map_or(Value::Null, |u| json!({
                "prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens,
                "cache_read_input_tokens":u.cache_read_input_tokens,"cache_creation_input_tokens":u.cache_creation_input_tokens
            })),
        },
        // Never reflect provider bodies or credentials in saved probe evidence.
        Err(_) => ModelProbeResult { status: None, usage: Value::Null,
            reply: Err("MiniMax M Plan test failed; invalid configuration, authentication or incomplete response.".into()) },
    }
}
