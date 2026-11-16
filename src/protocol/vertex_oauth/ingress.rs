//! Flattened hosted/cache input must not silently discard a credential-owned Full Vertex grant.
use super::{business, TOKEN_ENDPOINT};
use crate::{
    error::GatewayError,
    routing::{candidate::ProviderAccountPayload, config::ProviderCredentialYaml},
};
use serde_json::Value;

pub(crate) fn hydrate(
    payload: &mut ProviderAccountPayload,
    raw: Option<&Value>,
) -> Result<(), GatewayError> {
    let Some(raw) = raw else {
        return Ok(());
    };
    let has_context = raw.get("vertex_oauth").is_some_and(|v| !v.is_null());
    let typed = ["auth_mode", "authMode"]
        .iter()
        .any(|k| raw.get(k).and_then(Value::as_str) == Some(business::AUTH_MODE));
    if !has_context && !typed {
        return Ok(());
    }
    let config: ProviderCredentialYaml =
        serde_json::from_value(raw.clone()).map_err(|_| business::invalid())?;
    if config
        .auth_mode
        .as_deref()
        .is_some_and(|m| m != business::AUTH_MODE)
        || !typed && !business::owns_payload(payload)
        || config.auth_token.is_some()
        || config.grok_oauth.is_some()
        || config.kiro_oauth.is_some()
        || config.accio_sgcookie.is_some()
        || config.discovery.is_some()
        || config.discovery_job.is_some()
    {
        return Err(business::invalid());
    }
    payload.auth_mode = Some(business::AUTH_MODE.into());
    if let Some(context) = &config.vertex_oauth {
        context.validate().map_err(|_| business::invalid())?;
        if config
            .refresh_endpoint
            .as_deref()
            .is_some_and(|s| s != TOKEN_ENDPOINT)
            || config
                .refresh_client_id
                .as_deref()
                .is_some_and(|s| s != context.client_id())
            || config
                .refresh_token
                .as_deref()
                .is_some_and(|s| !business::valid_token(s))
            || payload
                .headers
                .get("x-goog-user-project")
                .is_some_and(|q| Some(q.as_str()) != context.target().quota_project())
        {
            return Err(business::invalid());
        }
        if let Some(quota) = context.target().quota_project() {
            payload
                .headers
                .insert("x-goog-user-project".into(), quota.into());
        }
        business::validate_context(payload, context)?;
    }
    business::validate(payload, payload.canonical_adapter())
}
