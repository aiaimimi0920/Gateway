//! Only documented SAP origins receive service secrets; deployment identity never comes from model text.
use super::*;
use crate::protocol::call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter};
use serde::Deserialize;
pub struct Endpoint {
    pub origin: String,
    pub deployment: String,
}
fn atom(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn url(value: &str) -> Option<url::Url> {
    let parsed = url::Url::parse(value).ok()?;
    (parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.port().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && !value.contains(['%', '\\'])
        && value == parsed.as_str().trim_end_matches('/'))
    .then_some(parsed)
}
fn api_origin(value: &str) -> Option<String> {
    let parsed = url(value)?;
    let region = parsed
        .host_str()?
        .strip_prefix("api.ai.prod.")?
        .strip_suffix(".aws.ml.hana.ondemand.com")?;
    (atom(region, 32) && matches!(parsed.path(), "/" | "/v2"))
        .then(|| parsed.origin().ascii_serialization())
}
impl Endpoint {
    pub fn parse(value: &str) -> Option<Self> {
        let parsed = url(value)?;
        let origin = api_origin(&parsed.origin().ascii_serialization())?;
        let deployment = parsed.path().strip_prefix("/v2/inference/deployments/")?;
        atom(deployment, 128).then(|| Self {
            origin,
            deployment: deployment.into(),
        })
    }
}
// Deserialize known fields directly so duplicate identity/secret fields are rejected.
#[derive(Deserialize)]
pub struct ServiceKey {
    pub clientid: String,
    pub clientsecret: String,
    pub url: String,
    pub serviceurls: ServiceUrls,
}
#[derive(Deserialize)]
pub struct ServiceUrls {
    #[serde(rename = "AI_API_URL")]
    pub api: String,
}
impl ServiceKey {
    pub fn parse(raw: &str, endpoint: &Endpoint) -> Result<Self, GatewayError> {
        if raw.len() > 65_536 {
            return Err(invalid());
        }
        let key: Self = serde_json::from_str(raw).map_err(|_| invalid())?;
        let auth = url(key.url.trim_end_matches('/')).ok_or_else(invalid)?;
        let host = auth.host_str().ok_or_else(invalid)?;
        let (tenant, region) = host
            .strip_suffix(".hana.ondemand.com")
            .and_then(|h| h.split_once(".authentication."))
            .ok_or_else(invalid)?;
        if !atom(tenant, 200)
            || !atom(region, 32)
            || auth.path() != "/"
            || api_origin(key.serviceurls.api.trim_end_matches('/')).as_deref()
                != Some(&endpoint.origin)
            || !secret_atom(&key.clientid, 2048)
            || !secret_atom(&key.clientsecret, 16_384)
        {
            return Err(invalid());
        }
        Ok(key)
    }
}
fn secret_atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| b.is_ascii_graphic())
}
pub fn validate(payload: &ProviderAccountPayload, adapter: &str) -> Result<(), GatewayError> {
    if !owns_payload(payload) {
        return Ok(());
    }
    crate::implementation_lines::ensure_payload_compiled(payload)?;
    let endpoint = Endpoint::parse(&payload.base_url).ok_or_else(invalid)?;
    if adapter != "openai_compatible"
        || payload.adapter != adapter
        || payload.auth_header_name.is_some()
        || payload.auth_token.is_some()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload.headers.len() != 1
        || !payload
            .headers
            .get("AI-Resource-Group")
            .is_some_and(|v| atom(v, 253))
        || payload.extra_body.as_ref().is_none_or(|b| {
            b.len() != 1
                || !b
                    .get("sap_api_version")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|v| {
                        let date = v.strip_suffix("-preview").unwrap_or(v);
                        date.len() == 10
                            && time::Date::parse(
                                date,
                                &time::format_description::parse("[year]-[month]-[day]")
                                    .expect("valid date format"),
                            )
                            .is_ok()
                    })
        })
        || !payload.discovered_protocols.is_empty()
        || payload
            .execution_mode
            .is_some_and(|m| m != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
        || payload.endpoint_execution_modes.as_ref().is_some_and(|m| {
            m.values()
                .any(|v| *v != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
        })
        || !matches!(
            payload.request_client_profile,
            None | Some(crate::protocol::request_profile::RequestClientProfile::Auto)
        )
        || payload.call_flow.is_some_and(|f| {
            !matches!(f.call_adapter, CallAdapter::Auto | CallAdapter::Openai)
                || f.fingerprint_adapter != FingerprintAdapter::Auto
                || !matches!(
                    f.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::None
                )
        })
        || [
            &payload.chat_completions_path,
            &payload.responses_path,
            &payload.messages_path,
            &payload.completions_path,
            &payload.embeddings_path,
            &payload.audio_transcriptions_path,
            &payload.audio_speech_path,
            &payload.search_path,
            &payload.fetch_path,
            &payload.research_path,
            &payload.balance_path,
        ]
        .iter()
        .any(|v| v.is_some())
    {
        return Err(invalid());
    }
    if !payload.api_key.is_empty() {
        ServiceKey::parse(&payload.api_key, &endpoint)?;
    }
    Ok(())
}
pub fn validate_declaration(
    cfg: &crate::routing::config::ProviderConfigYaml,
) -> anyhow::Result<()> {
    if cfg.preset.as_deref() != Some(PRESET)
        && cfg.auth_mode.as_deref() != Some(AUTH_MODE)
        && !cfg
            .credentials
            .iter()
            .any(|c| c.auth_mode.as_deref() == Some(AUTH_MODE))
    {
        return Ok(());
    }
    anyhow::ensure!(
        Endpoint::parse(&cfg.base_url).is_some()
            && cfg.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
            && cfg.credentials.iter().all(|c| c.refresh_token.is_none()
                && c.discovery_job.is_none()
                && c.discovery.is_none()
                && c.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
                && c.base_url.as_deref().is_none_or(|b| b == cfg.base_url)),
        "SAP AI Core service key cannot change its deployment or use a refresh token."
    );
    Ok(())
}

/// Keep must not silently grant the stored client secret access to another resource group.
pub fn config_binding(
    provider: &crate::routing::config::ProviderConfigYaml,
    credential: Option<&crate::routing::config::ProviderCredentialYaml>,
) -> Option<serde_json::Value> {
    if provider.preset.as_deref() != Some(PRESET)
        && credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            != Some(AUTH_MODE)
    {
        return None;
    }
    Some(serde_json::json!({
        "group": credential.and_then(|c| c.headers.get("AI-Resource-Group")).or_else(|| provider.headers.get("AI-Resource-Group")),
        "version": credential.and_then(|c| c.extra_body.get("sap_api_version")).or_else(|| provider.extra_body.get("sap_api_version"))
    }))
}
