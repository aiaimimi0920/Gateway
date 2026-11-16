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
) -> Result<(u16, CanonicalRelayResponse), GatewayError> {
    let request = openai::normalize_chat_completions(json!({
        "model":model,"stream":false,"max_tokens":256,"n":1,
        "messages":[{"role":"user","content":prompt}]
    }))?;
    // Same deployment admission, token owner, bounded exchange and no redirects/replay as inference.
    let (response, token) = runtime::send(client, payload, &request, model, false).await?;
    let status = response.status().as_u16();
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(),
            "SAP model test",
            65_536,
            None,
        )
        .await
        .map_err(|_| failure("sap_aicore_probe_response_failed"))?;
    let body: Value = crate::protocol::unique_json::parse(&bytes)
        .map_err(|_| failure("sap_aicore_probe_response_failed"))?;
    let mut result = openai::chat_admission::unpack(&body)
        .map_err(|_| failure("sap_aicore_probe_response_failed"))?;
    if body["choices"].as_array().is_none_or(|v| v.len() != 1)
        || body["choices"][0]["message"]
            .get("refusal")
            .is_some_and(|v| !v.is_null())
        || result.finish_reason.as_deref() != Some("stop")
        || !result.tool_calls.is_empty()
        || result.text.trim().is_empty()
        || result.text.chars().count() > 8192
    {
        return Err(failure("sap_aicore_probe_incomplete"));
    }
    // Redact individual fields as well as the full JSON, before previews, scoring or storage.
    let endpoint = Endpoint::parse(&payload.base_url).ok_or_else(invalid)?;
    let key = ServiceKey::parse(&payload.api_key, &endpoint)?;
    let mut secrets = vec![
        payload.api_key.as_str(),
        key.clientid.as_str(),
        key.clientsecret.as_str(),
        token.as_str(),
    ];
    secrets.sort_unstable_by_key(|s| std::cmp::Reverse(s.len()));
    secrets.dedup();
    for secret in secrets {
        result.text = result.text.replace(secret, "[redacted]");
    }
    result.text = crate::error::sanitize_provider_error_message(&result.text);
    Ok((status, result))
}
