//! Exact-account ConverseStream proof. Full native EOF precedes health/audit success.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    protocol::bedrock_converse, routing::config::CredentialProbeTarget, state::AppState,
    upstream::response_types::UpstreamStreamingResponse,
};
use futures::StreamExt;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("Bedrock ConverseStream model call (model: {model})");
    let audit = super::model_probe_recording::begin_bedrock(state, target, model).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Model call timed out after 60 seconds."));
    let recovery = super::model_probe_recording::finish(
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
                "Model call passed. Model: {model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("Model call failed. Model: {model}. {error}"),
        ),
    };
    (
        point,
        ProviderPayloadProbeReport {
            health_recovery: Some(recovery),
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
) -> ModelProbeResult {
    // Use native inferenceConfig, not OpenAI max_tokens which is not a Bedrock limit.
    // Caller-selected model/credentials remain unchanged; no catalog call, fallback or retry.
    let request = bedrock_converse::normalize_converse(
        json!({
            "messages":[{"role":"user","content":[{"text":prompt}]}],
            "inferenceConfig":{"maxTokens":32}
        }),
        Some(model.into()),
        true,
    );
    let Ok(request) = request else {
        return failed(None, "Cannot build the Bedrock model test request.");
    };
    let response = match state
        .upstream_client
        .execute_stream_with_wire(&target.payload, &request, model, None, None)
        .await
    {
        Ok(UpstreamStreamingResponse::Http(response)) => response,
        Ok(_) => return failed(None, "Bedrock test did not return native HTTP EventStream."),
        Err(error) => {
            return failed(
                error.http_status,
                &format!(
                    "Bedrock model request failed ({:?}, HTTP {}).",
                    error.kind,
                    error
                        .http_status
                        .map_or_else(|| "unknown".into(), |status| status.to_string())
                ),
            )
        }
    };
    let status = response.status().as_u16();
    if !response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| {
            v.trim()
                .eq_ignore_ascii_case("application/vnd.amazon.eventstream")
        })
    {
        return failed(
            Some(status),
            "Bedrock test requires native AWS EventStream content type.",
        );
    }
    let stream = response.bytes_stream();
    futures::pin_mut!(stream);
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if bytes.len().saturating_add(chunk.len()) <= 262_144 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(_) => return failed(Some(status), "Bedrock model response exceeded 256 KiB."),
            Err(_) => return failed(Some(status), "Bedrock model response was interrupted."),
        }
    }
    // Share the production CRC/header/block/stop/metadata/EOF decoder, never an SSE parser.
    let response = match bedrock_converse::decode_eventstream(bytes, model) {
        Ok(response) => response,
        Err(error) => {
            return failed(
                Some(status),
                if error.is_upstream_failure() {
                    "Bedrock returned a stream error."
                } else {
                    "Invalid or incomplete Bedrock EventStream."
                },
            )
        }
    };
    if response.finish_reason.as_deref() != Some("stop")
        || !response.tool_calls.is_empty()
        || response.text.trim().is_empty()
    {
        return failed(
            Some(status),
            "No completed non-empty text-only assistant reply was returned.",
        );
    }
    if response.text.chars().count() > 8192 {
        return failed(Some(status), "Bedrock test reply exceeded 8192 characters.");
    }
    let Some(usage) = response.usage else {
        return failed(Some(status), "Bedrock response omitted validated usage.");
    };
    ModelProbeResult {
        status: Some(status),
        reply: Ok(redact_reply(response.text, target)),
        usage: json!({
            "prompt_tokens":usage.prompt_tokens,"completion_tokens":usage.completion_tokens,"total_tokens":usage.total_tokens,
            "cache_read_input_tokens":usage.cache_read_input_tokens,"cache_creation_input_tokens":usage.cache_creation_input_tokens
        }),
    }
}

fn redact_reply(mut reply: String, target: &CredentialProbeTarget) -> String {
    let mut secrets: Vec<_> = std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.auth_token.as_deref())
        .chain(target.payload.headers.values().map(String::as_str))
        .chain(
            target
                .payload
                .extra_body
                .iter()
                .flatten()
                .filter(|(key, _)| crate::console::secrets::is_sensitive_key(key))
                .filter_map(|(_, value)| value.as_str()),
        )
        .flat_map(|secret| [secret, secret.trim()])
        .filter(|secret| !secret.is_empty())
        .collect();
    // Longer overlapping STS/SK values must be removed before their shorter substrings.
    secrets.sort_unstable_by_key(|secret| std::cmp::Reverse(secret.len()));
    secrets.dedup();
    for secret in secrets {
        reply = reply.replace(secret, "[redacted]");
    }
    crate::error::sanitize_provider_error_message(&reply)
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
