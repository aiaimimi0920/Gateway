//! Only documented SAP origins receive service secrets; deployment identity never comes from model text.
use super::*;
use crate::protocol::call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter};
pub struct Endpoint {
    pub origin: String,
    pub deployment: String,
}
pub(super) fn atom(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub(super) fn url(value: &str) -> Option<url::Url> {
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
pub(super) fn api_origin(value: &str) -> Option<String> {
    let parsed = url(value)?;
    let host = parsed.host_str()?;
    let supported = host == "api.ai.prod-eu20.westeurope.azure.ml.hana.ondemand.com"
        || host
            .strip_prefix("api.ai.prod.")
            .and_then(|host| host.strip_suffix(".aws.ml.hana.ondemand.com"))
            .is_some_and(|region| atom(region, 32));
    (supported && matches!(parsed.path(), "/" | "/v2"))
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
pub(super) const IMAGE_API_VERSION: &str = "2024-10-21";
pub(crate) const IMAGE_INPUT: &str = "sap_image_input";
pub(crate) const NATIVE_RESPONSES: &str = "sap_native_responses";
pub(crate) const NATIVE_RESPONSES_IMAGE_INPUT: &str = "sap_native_responses_image_input";
pub(crate) const STORED_RESPONSES: &str = "sap_stored_responses";
pub(crate) const O3_MINI: &str = "sap_o3_mini";
const O3_MINI_API_VERSION: &str = "2024-12-01-preview";

fn valid_capability(
    settings: &std::collections::HashMap<String, serde_json::Value>,
    key: &str,
    version: &str,
) -> bool {
    settings
        .get(key)
        .is_none_or(|enabled| match enabled.as_bool() {
            Some(false) => true,
            Some(true) => {
                settings
                    .get("sap_api_version")
                    .and_then(serde_json::Value::as_str)
                    == Some(version)
            }
            None => false,
        })
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
            if let Some(enabled) = b.get(BEDROCK_CONVERSE) {
                return enabled.as_bool() != Some(true)
                    || b.keys().any(|key| {
                        !matches!(key.as_str(), BEDROCK_CONVERSE | BEDROCK_IMAGE_INPUT)
                    })
                    || b.get(BEDROCK_IMAGE_INPUT)
                        .is_some_and(|flag| !flag.is_boolean());
            }
            if let Some(model) = b.get(ORCHESTRATION_MODEL) {
                // Orchestration and Azure OpenAI use different endpoints and request contracts.
                return !model.as_str().is_some_and(orchestration::valid_model)
                    || b.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            ORCHESTRATION_MODEL
                                | ORCHESTRATION_IMAGE_INPUT
                                | ORCHESTRATION_REASONING
                        )
                    })
                    || [ORCHESTRATION_IMAGE_INPUT, ORCHESTRATION_REASONING]
                        .iter()
                        .any(|&flag| b.get(flag).is_some_and(|v| !v.is_boolean()));
            }
            b.keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "sap_api_version"
                        | IMAGE_INPUT
                        | O3_MINI
                        | NATIVE_RESPONSES
                        | NATIVE_RESPONSES_IMAGE_INPUT
                        | STORED_RESPONSES
                )
            }) || b.get(NATIVE_RESPONSES).is_some_and(|v| !v.is_boolean())
                || [NATIVE_RESPONSES_IMAGE_INPUT, STORED_RESPONSES]
                    .into_iter()
                    .any(|flag| {
                        b.get(flag).is_some_and(|v| match v.as_bool() {
                            Some(false) => false,
                            Some(true) => {
                                b.get(NATIVE_RESPONSES).and_then(serde_json::Value::as_bool)
                                    != Some(true)
                            }
                            None => true,
                        })
                    })
                || !valid_capability(b, IMAGE_INPUT, IMAGE_API_VERSION)
                || !valid_capability(b, O3_MINI, O3_MINI_API_VERSION)
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
                && !c.extra_body.contains_key(IMAGE_INPUT)
                && !c.extra_body.contains_key(O3_MINI)
                && !c.extra_body.contains_key(NATIVE_RESPONSES)
                && !c.extra_body.contains_key(NATIVE_RESPONSES_IMAGE_INPUT)
                && !c.extra_body.contains_key(STORED_RESPONSES)
                && !c.extra_body.contains_key(ORCHESTRATION_MODEL)
                && !c.extra_body.contains_key(ORCHESTRATION_IMAGE_INPUT)
                && !c.extra_body.contains_key(ORCHESTRATION_REASONING)
                && !c.extra_body.contains_key(BEDROCK_CONVERSE)
                && !c.extra_body.contains_key(BEDROCK_IMAGE_INPUT)
                && (!cfg.extra_body.contains_key(ORCHESTRATION_MODEL)
                    && !cfg.extra_body.contains_key(BEDROCK_CONVERSE)
                    && cfg.extra_body.get(NATIVE_RESPONSES).and_then(serde_json::Value::as_bool) != Some(true)
                    || (c.headers.get("AI-Resource-Group")
                        .is_none_or(|v| Some(v) == cfg.headers.get("AI-Resource-Group"))
                        && c.extra_body.get("sap_api_version")
                            .is_none_or(|v| Some(v) == cfg.extra_body.get("sap_api_version"))))
                && c.auth_mode.as_deref().is_none_or(|m| m == AUTH_MODE)
                && c.base_url.as_deref().is_none_or(|b| b == cfg.base_url)),
        "SAP AI Core credentials cannot override deployment capability or its binding, change deployment or use a refresh token."
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
    let mut binding = serde_json::json!({
        "group": credential.and_then(|c| c.headers.get("AI-Resource-Group")).or_else(|| provider.headers.get("AI-Resource-Group")),
        "version": credential.and_then(|c| c.extra_body.get("sap_api_version")).or_else(|| provider.extra_body.get("sap_api_version"))
    });
    if let Some(model) = provider.extra_body.get(ORCHESTRATION_MODEL) {
        binding["orchestrationModel"] = model.clone();
    }
    if let Some(enabled) = provider.extra_body.get(BEDROCK_CONVERSE) {
        binding["bedrockConverse"] = enabled.clone();
    }
    Some(binding)
}
