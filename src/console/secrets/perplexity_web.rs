//! UA and absolute expiry are part of this site's session identity, never silently rebound by Keep.
pub(super) use crate::routing::config::perplexity_web::owns as owns_product;
use crate::routing::config::{perplexity_web::owns, ProviderConfigYaml, ProviderCredentialYaml};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn binding(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> Value {
    if !owns(provider, credential) {
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
    json!({"user_agent":headers, "expires_at":credential.and_then(|c| c.expires_at.as_ref()).or(provider.expires_at.as_ref())})
}
