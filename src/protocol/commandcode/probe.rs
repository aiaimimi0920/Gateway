//! Explicit text/decision tests share the production sender; credentials never escape in previews.
use super::*;
use crate::{
    protocol::{canonical::CanonicalRelayResponse, openai},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};

pub(crate) async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    model: &str,
    prompt: &str,
) -> Result<(u16, CanonicalRelayResponse), GatewayError> {
    validate(payload, payload.canonical_adapter())?;
    let endpoint = catalog::probe_endpoint(payload, model)?;
    let mut selected = payload.clone();
    selected.chat_completions_path =
        (endpoint == EndpointKind::ChatCompletions).then(|| "/chat/completions".into());
    selected.responses_path = (endpoint == EndpointKind::Responses).then(|| "/responses".into());
    selected.messages_path = (endpoint == EndpointKind::Messages).then(|| "/messages".into());
    let request = match endpoint {
        EndpointKind::SystemOne => super::systemone_probe::request(model, prompt)?,
        EndpointKind::Messages => crate::protocol::anthropic::normalize_messages(json!({
            "model":model,"stream":false,"max_tokens":256,"messages":[{"role":"user","content":prompt}]
        }))?,
        EndpointKind::Responses => crate::protocol::responses::normalize_responses(json!({
            "model":model,"stream":false,"max_output_tokens":256,"input":prompt
        }))?,
        _ => openai::normalize_chat_completions(json!({
            "model":model,"stream":false,"max_tokens":256,"n":1,"messages":[{"role":"user","content":prompt}]
        }))?,
    };
    // Same fixed public API, exact account Key and no redirects/replay as inference.
    let response = runtime::send(client, &selected, &request, model, false).await?;
    let status = response.status().as_u16();
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(),
            "Command Code model test",
            65_536,
            None,
        )
        .await
        .map_err(|_| response_error())?;
    let body: Value = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    let mut result = match endpoint {
        EndpointKind::SystemOne => super::systemone_probe::unpack(&body, &request.raw_body)?,
        EndpointKind::Messages => super::messages::unpack(&body)?,
        EndpointKind::Responses => super::responses::unpack(&body)?,
        _ => {
            if body["choices"].as_array().is_none_or(|v| v.len() != 1)
                || body["choices"][0]["message"]
                    .get("refusal")
                    .is_some_and(|v| !v.is_null())
            {
                return Err(response_error());
            }
            openai::chat_admission::unpack(&body).map_err(|_| response_error())?
        }
    };
    let finished = if endpoint == EndpointKind::Responses {
        "completed"
    } else {
        "stop"
    };
    if result.finish_reason.as_deref() != Some(finished)
        || !result.tool_calls.is_empty()
        || result.text.trim().is_empty()
        || result.text.chars().count() > 8192
    {
        return Err(response_error());
    }
    // Redact before previews, scoring and persistence; the validated Key is nonempty.
    result.text = result.text.replace(&payload.api_key, "[redacted]");
    result.text = crate::error::sanitize_provider_error_message(&result.text);
    Ok((status, result))
}
