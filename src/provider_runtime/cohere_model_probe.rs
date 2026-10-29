//! Explicit Cohere prompt proof through the production dispatcher and native decoder.
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde_json::{json, Value};

use super::console_model_probe::ModelProbeResult;
use super::{ProviderPayloadProbeReport, ProviderPayloadProbeStatus};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::upstream::{client::UpstreamClient, response_types::UpstreamStreamingResponse};
use crate::{routing::config::CredentialProbeTarget, state::AppState};

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    prompt: &str,
) -> (String, ProviderPayloadProbeReport) {
    let request = crate::protocol::openai::normalize_chat_completions(json!({
        "model": model, "stream": true, "max_tokens": 32,
        "messages": [{"role":"user", "content":prompt}]
    }));
    let point = request
        .as_ref()
        .ok()
        .and_then(|request| {
            UpstreamClient::build_request_plan(&target.payload, request, model, true).ok()
        })
        .map_or_else(
            || format!("Model call (model: {model})"),
            |plan| {
                format!(
                    "POST {} (model: {model})",
                    crate::console::secrets::redact_url_value(&plan.url)
                )
            },
        );
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(60), async {
        match request {
            Ok(request) => generate(state, target, &request, model).await,
            Err(_) => failed(None, "Cannot build the Cohere model test request."),
        }
    })
    .await
    .unwrap_or_else(|_| failed(None, "Model call timed out after 60 seconds."));
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
    request: &CanonicalRelayRequest,
    model: &str,
) -> ModelProbeResult {
    // The exact payload owns authentication, call flow and transport; no rerouting or fallback.
    let response = match state
        .upstream_client
        .execute_stream_with_wire(&target.payload, request, model, None, None)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return failed(
                error.http_status,
                &format!(
                    "Cohere model request failed ({:?}, HTTP {}).",
                    error.kind,
                    error
                        .http_status
                        .map_or_else(|| "unknown".into(), |status| status.to_string()),
                ),
            )
        }
    };
    type ProbeStream =
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, ()>> + Send>>;
    let (status, mut stream): (u16, ProbeStream) = match response {
        UpstreamStreamingResponse::Http(response) => (
            response.status().as_u16(),
            Box::pin(response.bytes_stream().map(|chunk| chunk.map_err(|_| ()))),
        ),
        UpstreamStreamingResponse::Native(response) => (
            response.status().as_u16(),
            Box::pin(response.bytes_stream().map(|chunk| chunk.map_err(|_| ()))),
        ),
        UpstreamStreamingResponse::NativeEvents(stream) => {
            (200, Box::pin(stream.map(|chunk| chunk.map_err(|_| ()))))
        }
        UpstreamStreamingResponse::Bytes(stream) => {
            (200, Box::pin(stream.map(|chunk| chunk.map_err(|_| ()))))
        }
    };
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if bytes.len().saturating_add(chunk.len()) <= 262_144 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(_) => return failed(Some(status), "Model response exceeded 256 KiB."),
            Err(_) => return failed(Some(status), "Model response was interrupted."),
        }
    }
    let Ok(response) = crate::protocol::cohere::native::decode_sse(&bytes, model) else {
        return failed(
            Some(status),
            "No completed non-empty assistant reply was returned.",
        );
    };
    if response.text.trim().is_empty() || response.finish_reason.as_deref() != Some("stop") {
        return failed(
            Some(status),
            "No completed non-empty assistant reply was returned.",
        );
    }
    let usage = response.usage.map_or(Value::Null, |usage| json!({
        "prompt_tokens":usage.prompt_tokens,"completion_tokens":usage.completion_tokens,"total_tokens":usage.total_tokens,
    }));
    let mut reply = response.text;
    for secret in std::iter::once(target.payload.api_key.as_str())
        .chain(target.payload.auth_token.as_deref())
        .chain(target.payload.headers.values().map(String::as_str))
        .filter(|value| !value.is_empty())
    {
        reply = reply.replace(secret, "[redacted]");
    }
    ModelProbeResult {
        status: Some(status),
        usage,
        reply: Ok(crate::error::sanitize_provider_error_message(&reply)
            .chars()
            .take(8192)
            .collect()),
    }
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
