//! Ordinary Qwen tests share the product planner/header/sender, not generic connectivity.
use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    routing::config::CredentialProbeTarget, state::AppState, upstream::client::UpstreamClient,
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
    let point = crate::provider_discovery::model_catalog::qwen_endpoint::QwenEndpoint::parse(
        &target.payload.base_url,
    )
    .map_or_else(
        || "Qwen workspace Chat model call".into(),
        |endpoint| {
            format!(
                "POST {}/chat/completions (model: {model})",
                endpoint.base_url
            )
        },
    );
    let audit = super::model_probe_recording::begin(state, target, model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        generate(state, target, model, prompt),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Qwen model call timed out after 60 seconds."));
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
    model: &str,
    prompt: &str,
) -> ModelProbeResult {
    let Ok(flow) = crate::protocol::call_flow::prepare(&target.payload) else {
        return failed(None, "Invalid Qwen workspace call flow; call not sent.");
    };
    let payload = flow.payload.as_ref();
    if flow.executor != "direct_http" || flow.call_adapter != "openai_compatible" {
        return failed(
            None,
            "Qwen workspace tests require the ordinary Chat API; call not sent.",
        );
    }
    let Ok(request) = crate::protocol::openai::normalize_chat_completions(json!({
        "model":model, "stream":false, "max_tokens":256, "n":1,
        "messages":[{"role":"user", "content":prompt}]
    })) else {
        return failed(
            None,
            "Cannot build the Qwen model test input; call not sent.",
        );
    };
    // The formal planner retains workspace/feature/key checks and fingerprint plan post-processing.
    let Ok(plan) = UpstreamClient::build_request_plan(payload, &request, model, false) else {
        return failed(
            None,
            "Invalid Qwen workspace request or credential; call not sent.",
        );
    };
    #[cfg(test)]
    let plan = test_transport::rewrite(plan);
    let headers = crate::upstream::headers::build_inference_headers(payload, &request, model, None);
    let Ok(pending) = state
        .upstream_client
        .send_profiled_plan(payload, &plan, headers)
    else {
        return failed(
            None,
            "Cannot prepare the Qwen model transport; call not sent.",
        );
    };
    // No routing, retries or account fallback. The sender also forbids redirect replay.
    let Ok(response) = pending.send().await else {
        return failed(None, "Cannot reach the Qwen model endpoint.");
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
        // Never expose response bodies, transport errors or credentials in failure evidence.
        return failed(Some(status), &format!("HTTP {status}: {reason}."));
    }
    let stream = response.bytes_stream();
    futures::pin_mut!(stream);
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if chunk.len() <= 65_536usize.saturating_sub(bytes.len()) => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(_) => return failed(Some(status), "Qwen model response exceeded 64 KiB."),
            Err(_) => return failed(Some(status), "Qwen model response was interrupted."),
        }
    }
    let Ok(body) = serde_json::from_slice::<Value>(&bytes) else {
        return failed(Some(status), "Qwen model endpoint returned invalid JSON.");
    };
    let message = &body["choices"][0]["message"];
    let content = message["content"].as_str().map(str::trim);
    if body.get("error").is_some_and(|v| !v.is_null())
        || body["choices"][0]["finish_reason"].as_str() != Some("stop")
        || message["role"].as_str() != Some("assistant")
        || content.is_none_or(str::is_empty)
        || message.get("refusal").is_some_and(|v| !v.is_null())
        || message.get("function_call").is_some_and(|v| !v.is_null())
        || message
            .get("tool_calls")
            .is_some_and(|v| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
    {
        return failed(
            Some(status),
            "No completed non-empty text-only assistant reply was returned.",
        );
    }
    let mut reply = content.unwrap_or_default().to_owned();
    for secret in std::iter::once(payload.api_key.as_str())
        .chain(payload.auth_token.as_deref())
        .chain(payload.headers.values().map(String::as_str))
        .filter(|v| !v.is_empty())
    {
        reply = reply.replace(secret, "[redacted]");
    }
    ModelProbeResult {
        status: Some(status),
        usage: body.get("usage").cloned().unwrap_or(Value::Null),
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

#[cfg(test)]
pub(super) mod test_transport {
    //! No production config or HTTP input can select a transport rewrite; each test owns its task.
    tokio::task_local! { static ORIGIN: String; }
    pub(in crate::provider_runtime) async fn within<T>(
        origin: &str,
        future: impl std::future::Future<Output = T>,
    ) -> T {
        ORIGIN.scope(origin.to_owned(), future).await
    }
    pub(super) fn rewrite(
        mut plan: crate::upstream::common::RequestPlan,
    ) -> crate::upstream::common::RequestPlan {
        let _ = ORIGIN.try_with(|origin| {
            let local = url::Url::parse(origin).unwrap();
            assert_eq!(local.scheme(), "http");
            assert_eq!(local.host_str(), Some("127.0.0.1"));
            let source = url::Url::parse(&plan.url).unwrap();
            plan.url = format!("{origin}/{}{}", source.host_str().unwrap(), source.path());
        });
        plan
    }
}
