//! Exact-account console calls reuse Lobster's catalog, sender and durable rejection owner.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    error::GatewayError, protocol::lobsterai, routing::config::CredentialProbeTarget,
    state::AppState,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let mut selection = None;
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt, &mut selection),
    )
    .await
    .unwrap_or_else(|_| failed(None, "LobsterAI model test timed out after 60 seconds."));
    let health = super::model_probe_recording::finish(
        state,
        target,
        model,
        audit.lobster_selection(selection.as_ref()),
        started.elapsed(),
        &result,
    )
    .await;
    let answer = result.reply.as_ref().ok().cloned();
    let (status, message) = match result.reply {
        Ok(reply) => (
            ProviderPayloadProbeStatus::Passed,
            format!(
                "LobsterAI model {model}: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (ProviderPayloadProbeStatus::Failed, error),
    };
    (
        format!("LobsterAI model test ({model})"),
        ProviderPayloadProbeReport {
            status,
            message,
            answer,
            health_recovery: Some(health),
        },
    )
}

async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
    selection: &mut Option<lobsterai::virtual_model::Selection>,
) -> ModelProbeResult {
    let request = match crate::protocol::openai::normalize_chat_completions(json!({
        "model":model,"stream":false,"max_tokens":256,"n":1,
        "messages":[{"role":"user","content":prompt}]
    })) {
        Ok(request) => request,
        Err(_) => return failed(None, "Invalid LobsterAI model test request."),
    };
    // Audit initialization and model resolution can yield. Never silently substitute a new token.
    if !current(state, target, model) {
        return failed(None, "LobsterAI credential binding changed; call not sent.");
    }
    let mut payload = target.payload.clone();
    let mut ticket = match lobsterai::dispatch::Ticket::capture_for_provider(
        state,
        &target.provider_id,
        &payload,
    ) {
        Ok(ticket) => ticket,
        Err(_) => return failed(None, "LobsterAI credential is not ready; call not sent."),
    };
    let response = async {
        *selection = lobsterai::virtual_model::prepare(
            &state.upstream_client,
            &mut payload,
            &request,
            model,
            &uuid::Uuid::new_v4().to_string(),
        )
        .await?;
        if !current(state, target, model) {
            return Err(GatewayError::bad_request(
                "LobsterAI credential binding changed before inference",
            ));
        }
        ticket = lobsterai::dispatch::Ticket::capture_for_provider(
            state,
            &target.provider_id,
            &payload,
        )?;
        lobsterai::runtime::execute_probe(&state.upstream_client, &payload, &request, model).await
    }
    .await;
    let mut response = match response {
        Ok(response) => response,
        Err(error) => {
            // Only a genuine sender 401 invalidates the captured generation; no probe retry.
            if let Some(ticket) = ticket {
                ticket.observe(state, &error);
            }
            return failed(error.http_status, "LobsterAI model test failed; invalid configuration, authentication or incomplete response.");
        }
    };
    if response.finish_reason.as_deref() != Some("stop")
        || !response.tool_calls.is_empty()
        || response.text.trim().is_empty()
        || response.text.chars().count() > 8192
    {
        return failed(
            None,
            "LobsterAI returned no complete non-empty text answer.",
        );
    }
    response.text = crate::error::sanitize_provider_error_message(
        &response.text.replace(&payload.api_key, "[redacted]"),
    );
    ModelProbeResult {
        // Canonical response metadata is not an authoritative HTTP status receipt.
        status: None,
        reply: Ok(response.text),
        usage: response.usage.map_or(Value::Null, |u| json!({
            "prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens,
            "cache_read_input_tokens":u.cache_read_input_tokens,"cache_creation_input_tokens":u.cache_creation_input_tokens
        })),
    }
}
fn current(state: &AppState, target: &CredentialProbeTarget, model: &str) -> bool {
    state
        .route_config
        .snapshot()
        .model_probe_identity(target, model)
        .is_some()
}
fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
