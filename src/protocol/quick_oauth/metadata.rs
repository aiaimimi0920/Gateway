//! Internal runtime metadata projected onto `ProviderAccountPayload` at route compile time.
//! External YAML/JSON callers must never inject `__gateway_quick_oauth` directly.

use super::{context::valid_ws_host, invalid_config, AuthorizationContext, AUTH_MODE, PORTAL_BASE};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde::{Deserialize, Serialize};

pub(crate) const METADATA_KEY: &str = "__gateway_quick_oauth";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeMetadata {
    pub ws_host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_arn: Option<String>,
}

pub(crate) fn attach(payload: &mut ProviderAccountPayload, context: &AuthorizationContext) {
    let meta = RuntimeMetadata {
        ws_host: context.ws_host().to_owned(),
        actor_arn: context.actor_arn().map(str::to_owned),
    };
    if let Ok(value) = serde_json::to_value(meta) {
        payload
            .extra_body
            .get_or_insert_with(Default::default)
            .insert(METADATA_KEY.to_owned(), value);
    }
}

pub(crate) fn read(payload: &ProviderAccountPayload) -> Result<RuntimeMetadata, GatewayError> {
    if payload.canonical_adapter() != "openai_compatible"
        || payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || payload.base_url.trim_end_matches('/') != PORTAL_BASE
        || !super::valid_token(&payload.api_key)
    {
        return Err(invalid_config());
    }
    let raw = payload
        .extra_body
        .as_ref()
        .and_then(|extra| extra.get(METADATA_KEY))
        .cloned()
        .ok_or_else(invalid_config)?;
    let meta: RuntimeMetadata = serde_json::from_value(raw).map_err(|_| invalid_config())?;
    if !valid_ws_host(&meta.ws_host)
        || meta.actor_arn.as_deref().is_some_and(|arn| {
            arn.is_empty() || arn.len() > 512 || arn.chars().any(char::is_control)
        })
    {
        return Err(invalid_config());
    }
    Ok(meta)
}

pub(crate) fn validate_payload(
    payload: &ProviderAccountPayload,
    context: &AuthorizationContext,
) -> Result<(), GatewayError> {
    let meta = read(payload)?;
    if meta.ws_host != context.ws_host() || meta.actor_arn.as_deref() != context.actor_arn() {
        return Err(invalid_config());
    }
    Ok(())
}
