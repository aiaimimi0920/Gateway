//! Explicit text tests share the production sender; credentials never escape in returned text.
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
    evidence: &probe_evidence::ProbeEvidence,
) -> Result<(u16, CanonicalRelayResponse), GatewayError> {
    let request = openai::normalize_chat_completions(json!({
        "model":model,"stream":false,"max_tokens":256,"n":1,
        "messages":[{"role":"user","content":prompt}]
    }))?;
    // Same deployment admission, token owner, bounded exchange and no redirects/replay as inference.
    let prepared = sender::Prepared::prepare(client, payload, &request, model, false).await?;
    evidence.begin();
    let (response, token) = prepared.send().await.map_err(|error| {
        if let Some(status) = errors::upstream_status(&error) {
            evidence.receive(status);
        }
        error
    })?;
    let status = response.status().as_u16();
    evidence.receive(status);
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(),
            "SAP model test",
            65_536,
            None,
        )
        .await
        .map_err(|_| failure("sap_aicore_body_incomplete"))?;
    let body: Value = crate::protocol::unique_json::parse(&bytes)
        .map_err(|_| failure("sap_aicore_probe_response_failed"))?;
    let mut result = if bedrock::selected(payload) {
        bedrock::unpack(&body, &request, model)
            .ok_or_else(|| failure("sap_aicore_probe_response_failed"))?
    } else {
        let body = if orchestration::model(payload).is_some() {
            orchestration::final_result(&body, false)
                .ok_or_else(|| failure("sap_aicore_probe_response_failed"))?
        } else {
            &body
        };
        if body["choices"].as_array().is_none_or(|v| v.len() != 1)
            || body["choices"][0]["message"]
                .get("refusal")
                .is_some_and(|v| !v.is_null())
        {
            return Err(failure("sap_aicore_probe_incomplete"));
        }
        openai::chat_admission::unpack(body)
            .map_err(|_| failure("sap_aicore_probe_response_failed"))?
    };
    if result.finish_reason.as_deref() != Some("stop")
        || !result.tool_calls.is_empty()
        || result.text.trim().is_empty()
        || result.text.chars().count() > 8192
    {
        return Err(failure("sap_aicore_probe_incomplete"));
    }
    // Redact individual fields as well as the full JSON, before previews, scoring or storage.
    let endpoint = Endpoint::parse(&payload.base_url).ok_or_else(invalid)?;
    let key = ServiceKey::parse(&payload.api_key, &endpoint)?;
    let mut secrets = key.secret_fragments();
    secrets.extend([payload.api_key.as_str(), token.as_str()]);
    secrets.sort_unstable_by_key(|s| std::cmp::Reverse(s.len()));
    secrets.dedup();
    for secret in secrets {
        result.text = result.text.replace(secret, "[redacted]");
    }
    result.text = crate::error::sanitize_provider_error_message(&result.text);
    Ok((status, result))
}
