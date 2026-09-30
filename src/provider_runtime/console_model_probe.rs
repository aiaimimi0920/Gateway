//! Explicit NVIDIA console tests require a generated answer from the selected credential.
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{ProviderPayloadProbeReport, ProviderPayloadProbeStatus};
use crate::routing::config::CredentialProbeTarget;
use crate::state::AppState;

pub(super) struct ModelProbeResult {
    pub status: Option<u16>,
    pub reply: Result<String, String>,
    pub usage: Value,
}

pub(crate) async fn probe_console_target(
    state: &AppState,
    target: &CredentialProbeTarget,
) -> (String, ProviderPayloadProbeReport) {
    let snapshot = state.route_config.snapshot();
    let nvidia = snapshot.document().providers.iter().any(|provider| {
        provider.id == target.provider_id && provider.preset.as_deref() == Some("nvidia-openai")
    }) || target.provider_id == "nvidia";
    let point = super::provider_payload_probe_point(&target.payload);
    if !target.enabled {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Credential is disabled.",
            ),
        );
    }
    let codex = snapshot.document().providers.iter().any(|provider| {
        provider.id == target.provider_id
            && provider.preset.as_deref() == Some("chatgpt-codex-oauth-official-api")
    });
    if codex {
        return super::codex_model_probe::probe(state, target).await;
    }
    if !nvidia {
        return (
            point,
            super::probe_provider_payload_for_console(
                state.upstream_client.client(),
                &target.payload,
            )
            .await,
        );
    }
    let provider = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == target.provider_id);
    let Some(provider) = provider else {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Provider is no longer configured.",
            ),
        );
    };
    let credential_models = provider
        .credential_pool
        .iter()
        .find(|credential| credential.id == target.credential_id)
        .map(|credential| credential.supported_models.as_slice())
        .unwrap_or_default();
    let model = target
        .payload
        .default_model
        .as_deref()
        .into_iter()
        .chain(provider.supported_models.iter().map(String::as_str))
        .chain(credential_models.iter().map(String::as_str))
        .map(str::trim)
        .find(|model| {
            !model.is_empty()
                && !model.contains(['*', '?'])
                && (credential_models.is_empty()
                    || credential_models.iter().any(|allowed| allowed == model))
        })
        .map(|model| {
            provider
                .model_map
                .get(model)
                .map(String::as_str)
                .unwrap_or(model)
        });
    let Some(model) = model else {
        return ("NVIDIA model call".into(), report(ProviderPayloadProbeStatus::Unsupported,
            "Configure a concrete default_model or supported_models entry before testing NVIDIA."));
    };
    let mut body = json!({"model": model, "stream": false, "max_tokens": 256,
        "messages": [{"role": "user", "content": "Reply with only OK."}]});
    if model.starts_with("nvidia/nemotron") {
        body["chat_template_kwargs"] = json!({"enable_thinking": false});
    }
    let plan = crate::protocol::openai::normalize_chat_completions(body).and_then(|request| {
        crate::upstream::openai_compatible_request_plan::build_request_plan(
            &target.payload,
            &request,
            model,
            false,
        )
    });
    let Ok(plan) = plan else {
        return (
            "NVIDIA model call".into(),
            report(
                ProviderPayloadProbeStatus::Failed,
                "Cannot build NVIDIA model test request.",
            ),
        );
    };
    let point = format!(
        "POST {} (model: {model})",
        crate::console::secrets::redact_url_value(&plan.url)
    );
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(60), generate(state, target, &plan))
        .await
        .unwrap_or_else(|_| ModelProbeResult {
            status: None,
            usage: Value::Null,
            reply: Err("Model call timed out after 60 seconds.".into()),
        });
    super::model_probe_recording::finish(
        state,
        target,
        model,
        audit.as_deref(),
        started.elapsed(),
        &result,
    )
    .await;
    let (status, message) = match result.reply {
        Ok(reply) => (
            ProviderPayloadProbeStatus::Passed,
            format!("Model call passed. Model: {model}. Reply: {reply}"),
        ),
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("Model call failed. Model: {model}. {error}"),
        ),
    };
    (point, ProviderPayloadProbeReport { status, message })
}

async fn generate(
    state: &AppState,
    target: &CredentialProbeTarget,
    plan: &crate::upstream::common::RequestPlan,
) -> ModelProbeResult {
    let request = crate::upstream::common::build_request_builder_from_plan(
        state.upstream_client.client(),
        Duration::from_secs(60),
        plan,
        crate::upstream::headers::build_upstream_headers(&target.payload),
    );
    let mut response = match request.send().await {
        Ok(response) => response,
        Err(_) => return failed(None, "Cannot reach the NVIDIA model endpoint."),
    };
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let reason = match status {
            401 => "Credential authentication rejected",
            403 => "Credential has no access to this model",
            404 => "Configured model or generation endpoint was not found",
            429 => "Upstream rate limit or quota exhausted",
            _ => "Upstream model request failed",
        };
        return failed(Some(status), &format!("HTTP {status}: {reason}."));
    }
    // Bound the complete response, not just the preview; never log upstream bodies or keys.
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= 65_536 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(Some(_)) => return failed(Some(status), "Model response exceeded 64 KiB."),
            Ok(None) => break,
            Err(_) => return failed(Some(status), "Model response was interrupted."),
        }
    }
    let Ok(body) = serde_json::from_slice::<Value>(&bytes) else {
        return failed(Some(status), "Model endpoint returned invalid JSON.");
    };
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim);
    if body.get("error").is_some_and(|value| !value.is_null()) || content.is_none_or(str::is_empty)
    {
        return failed(
            Some(status),
            "HTTP succeeded but no non-empty assistant reply was returned.",
        );
    }
    if body
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        == Some("length")
    {
        return failed(
            Some(status),
            "Model hit the test token limit before completing its reply.",
        );
    }
    let mut reply = content.unwrap_or_default().to_string();
    for secret in std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.auth_token.as_deref())
        .chain(target.payload.headers.values().map(String::as_str))
        .filter(|value| !value.is_empty())
    {
        reply = reply.replace(secret, "[redacted]");
    }
    let reply = crate::error::sanitize_provider_error_message(&reply)
        .chars()
        .take(320)
        .collect();
    ModelProbeResult {
        status: Some(status),
        reply: Ok(reply),
        usage: body.get("usage").cloned().unwrap_or(Value::Null),
    }
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}

fn report(status: ProviderPayloadProbeStatus, message: &str) -> ProviderPayloadProbeReport {
    ProviderPayloadProbeReport {
        status,
        message: message.into(),
    }
}
