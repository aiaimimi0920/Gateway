//! Explicit Accio generation uses the exact target and ordinary product dispatcher, never Chat JSON.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{routing::config::CredentialProbeTarget, state::AppState};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("Accio generateContent model call (model: {model})");
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let secrets = material_secrets(state, target);
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt, secrets),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Accio model call timed out after 60 seconds."));
    let health_recovery = super::model_probe_recording::finish(
        state,
        target,
        model,
        audit,
        started.elapsed(),
        &result,
    )
    .await;
    let answer = result.reply.as_ref().ok().cloned();
    let (status, message) = match result.reply {
        Ok(reply) => (
            ProviderPayloadProbeStatus::Passed,
            format!(
                "Accio model call passed. Model: {model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("Accio model call failed. Model: {model}. {error}"),
        ),
    };
    (
        point,
        ProviderPayloadProbeReport {
            health_recovery: Some(health_recovery),
            status,
            message,
            answer,
        },
    )
}
async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
    secrets: Vec<String>,
) -> ModelProbeResult {
    let request = crate::protocol::openai::normalize_chat_completions(json!({
        "model":model,"stream":false,"max_tokens":256,"messages":[{"role":"user","content":prompt}]
    }));
    let Ok(request) = request else {
        return failed(None, "Cannot build the Accio model test request.");
    };
    // No routing, account/channel fallback or retry. This retains normal call-flow and auth binding.
    let response = match state
        .upstream_client
        .execute_with_provider_account_id(
            &target.provider_id,
            &target.payload,
            &request,
            model,
            None,
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return failed(
                error.http_status,
                &format!(
                    "Accio request failed ({:?}, HTTP {}).",
                    error.kind,
                    error
                        .http_status
                        .map_or_else(|| "unknown".into(), |status| status.to_string())
                ),
            )
        }
    };
    if response.finish_reason.as_deref() != Some("stop")
        || !response.tool_calls.is_empty()
        || response.text.trim().is_empty()
    {
        return failed(
            response.upstream_status,
            "No completed non-empty text-only Accio reply was returned.",
        );
    }
    if response.text.chars().count() > 8192 {
        return failed(
            response.upstream_status,
            "Accio test reply exceeded 8192 characters.",
        );
    }
    let mut reply = response.text;
    for secret in secrets {
        reply = reply.replace(&secret, "[redacted]");
    }
    ModelProbeResult{status:response.upstream_status,reply:Ok(crate::error::sanitize_provider_error_message(&reply)),
        usage:response.usage.map_or(Value::Null,|usage|json!({"prompt_tokens":usage.prompt_tokens,"completion_tokens":usage.completion_tokens,
            "total_tokens":usage.total_tokens,"cache_read_input_tokens":usage.cache_read_input_tokens,"cache_creation_input_tokens":usage.cache_creation_input_tokens}))}
}
fn material_secrets(state: &AppState, target: &CredentialProbeTarget) -> Vec<String> {
    let mut secrets: Vec<String> = std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.auth_token.as_deref())
        .chain(target.payload.headers.values().map(String::as_str))
        .chain(
            target
                .payload
                .extra_body
                .iter()
                .flatten()
                .filter_map(|(_, value)| value.as_str()),
        )
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    // Refresh material is deliberately absent from dispatch payloads, but echoed text still must
    // not publish it. Capture only this target's configured material before awaiting generation.
    let snapshot = state.route_config.snapshot();
    if let Some(provider) = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == target.provider_id)
    {
        if let Some((_, credential)) =
            provider
                .credentials
                .iter()
                .enumerate()
                .find(|(index, credential)| {
                    crate::routing::config::effective_credential_id(
                        &target.provider_id,
                        *index,
                        credential,
                    ) == target.credential_id
                })
        {
            if let Some(refresh) = credential.refresh_token.as_ref() {
                secrets.push(refresh.clone());
            }
            if let Some(sg) = credential.accio_sgcookie.as_ref() {
                secrets.push(sg.clone());
            }
        }
    }
    for (key, value) in &target.payload.headers {
        if key.eq_ignore_ascii_case("cookie") {
            secrets.extend(
                value
                    .split(';')
                    .filter_map(|pair| pair.split_once('=').map(|(_, value)| value.trim()))
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned),
            );
        }
    }
    secrets.retain(|value| !value.is_empty());
    secrets.sort_unstable_by_key(|value| std::cmp::Reverse(value.len()));
    secrets.dedup();
    secrets
}
fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        usage: Value::Null,
        reply: Err(message.into()),
    }
}
