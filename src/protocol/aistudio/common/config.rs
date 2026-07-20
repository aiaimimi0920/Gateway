use serde_json::Value;

use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;

pub const AISTUDIO_DEFAULT_APP_URL: &str =
    "https://ai.studio/apps/fa9cb8e6-4d92-4fb6-a2b1-b947405c22ae";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AIStudioWebConfig {
    pub runtime_state_object_key: Option<String>,
    pub target_rpc_contract_object_key: Option<String>,
    pub app_url: String,
    pub fixture_transport: bool,
    pub browser_executable_path: Option<String>,
    pub cloud_api_key: Option<String>,
}

pub fn config_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<AIStudioWebConfig, GatewayError> {
    let extra = payload.extra_body.as_ref();
    let fixture_transport = extra
        .and_then(|body| {
            body.get("fixtureTransport")
                .or_else(|| body.get("fixture_transport"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.eq_ignore_ascii_case("direct_http"))
        .unwrap_or(false);
    let app_url = extra
        .and_then(|body| body.get("appUrl").or_else(|| body.get("app_url")))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(AISTUDIO_DEFAULT_APP_URL)
        .to_string();
    let browser_executable_path = extra
        .and_then(|body| {
            body.get("browserExecutablePath")
                .or_else(|| body.get("browser_executable_path"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let cloud_api_key = payload
        .api_key
        .trim()
        .is_empty()
        .then(|| {
            extra.and_then(|body| {
                body.get("cloudApiKey")
                    .or_else(|| body.get("cloud_api_key"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
        .flatten()
        .or_else(|| {
            let api_key = payload.api_key.trim();
            (!api_key.is_empty()).then(|| api_key.to_string())
        });
    let runtime_state_object_key = payload
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let target_rpc_contract_object_key = extra
        .and_then(|body| {
            body.get("targetRpcContractObjectKey")
                .or_else(|| body.get("target_rpc_contract_object_key"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Ok(AIStudioWebConfig {
        runtime_state_object_key,
        target_rpc_contract_object_key,
        app_url,
        fixture_transport,
        browser_executable_path,
        cloud_api_key,
    })
}
