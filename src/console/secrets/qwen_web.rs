//! Keep binds the current Web audience, optional original UA and absolute expiry; no seed migration.
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn owns_product(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    matches!(
        provider.preset.as_deref(),
        Some("qwen-web-chat" | "qwen-webui-replay-live")
    ) || provider.adapter.as_deref().is_some_and(|a| {
        crate::routing::candidate::canonicalize_adapter_name(a) == "qwen_web_compatible"
    }) || provider
        .call_flow
        .is_some_and(|f| f.special_adapter == crate::protocol::call_flow::SpecialAdapter::QwenWeb)
        || url::Url::parse(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
        .is_ok_and(|url| url.host_str() == Some("chat.qwen.ai"))
}
pub(super) fn binding(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> Value {
    if !owns_product(provider, credential) {
        return Value::Null;
    }
    let mut headers: BTreeMap<_, _> = provider
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
        .collect();
    if let Some(credential) = credential {
        headers.extend(
            credential
                .headers
                .iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case("user-agent")),
        );
    }
    json!({"user_agent":headers,"expires_at":credential.and_then(|c| c.expires_at.as_ref()).or(provider.expires_at.as_ref())})
}
