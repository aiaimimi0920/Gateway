//! Typed registrations cannot be routed through arbitrary hosts, legacy auth copies or header overrides.
use super::{directory, AuthorizationContext, OAuthError};
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};

pub(crate) fn valid_token(value: &str) -> bool {
    super::atom(value, 32_768)
}

fn key(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, '_' | '-'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
pub(crate) fn auth_extra(value: &str) -> bool {
    matches!(
        key(value).as_str(),
        "kirorefreshtoken"
            | "refreshtoken"
            | "kiroaccesstoken"
            | "accesstoken"
            | "kiroauthmethod"
            | "authmethod"
            | "kiroclientid"
            | "clientid"
            | "kiroclientsecret"
            | "clientsecret"
            | "kiroauthregion"
            | "authregion"
            | "kiroapiregion"
            | "apiregion"
            | "apikey"
            | "authtoken"
            | "authorization"
            | "cookie"
            | "token"
            | "kirooauth"
            | "grokoauth"
            | "acciosgcookie"
            | "rawsource"
    )
}
fn machine(value: &str) -> bool {
    matches!(key(value).as_str(), "kiromachineid" | "machineid")
}
pub(crate) fn validate_payload(
    payload: &ProviderAccountPayload,
    context: &AuthorizationContext,
) -> Result<(), OAuthError> {
    if context.runtime_target().is_some() || super::runtime::owns_payload(payload) {
        let target = super::RuntimeTarget::from_payload(payload)?;
        if context.runtime_target() != Some(&target) {
            return Err(OAuthError::InvalidInput);
        }
    }
    validate_material_payload(payload, context.machine_id())
}

/// Folder imports use the same final business boundary before storing a resolved device.
pub(crate) fn validate_material_payload(
    payload: &ProviderAccountPayload,
    machine_id: &str,
) -> Result<(), OAuthError> {
    if super::runtime::owns_payload(payload) {
        super::runtime::validate(payload).map_err(|_| OAuthError::InvalidInput)?;
        if payload.extra_body.as_ref().is_some_and(|body| {
            body.get("kiroMachineId")
                .is_some_and(|value| value.as_str() != Some(machine_id))
        }) {
            return Err(OAuthError::InvalidInput);
        }
        return Ok(());
    }
    let url = url::Url::parse(&payload.base_url).map_err(|_| OAuthError::InvalidInput)?;
    let regional = url
        .host_str()
        .and_then(|host| host.strip_suffix(".amazonaws.com"))
        .and_then(|host| host.split_once('.'));
    let regional = regional.is_some_and(|(service, region)| {
        matches!(service, "q" | "codewhisperer") && directory::commercial_region(region)
    });
    if payload.canonical_adapter() != "kiro_compatible"
        || !regional
        || url.scheme() != "https"
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
        || payload.auth_token.is_some()
        || payload.auth_header_name.is_some()
        || !payload.api_key.is_empty() && !valid_token(&payload.api_key)
        // The registered owner cannot delegate its bearer to a mutable external steward/browser.
        || payload.keepalive.is_some()
        || payload.execution_mode.is_some_and(|mode| mode != ProviderExecutionMode::DirectHttp)
        || payload.endpoint_execution_modes.as_ref().is_some_and(|modes| {
            modes.values().any(|mode| *mode != ProviderExecutionMode::DirectHttp)
        })
        || payload
            .responses_path
            .as_deref()
            .is_some_and(|path| path != "/generateAssistantResponse")
        || payload.session_auth.as_ref().is_none_or(|auth| {
            auth.transport != "bearer"
                || auth
                    .header_name()
                    .is_some_and(|name| !name.eq_ignore_ascii_case("authorization"))
        })
        || payload.headers.keys().any(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "authorization"
                    | "proxy-authorization"
                    | "cookie"
                    | "x-api-key"
                    | "api-key"
                    | "x-goog-api-key"
                    | "host"
                    | "user-agent"
                    | "x-amz-user-agent"
            )
        })
        || payload.extra_body.as_ref().is_some_and(|extra| {
            extra.iter().any(|(name, value)| {
                auth_extra(name) || machine(name) && value.as_str() != Some(machine_id)
            })
        })
    {
        return Err(OAuthError::InvalidInput);
    }
    Ok(())
}
pub(crate) fn configure(
    payload: &mut ProviderAccountPayload,
    context: &AuthorizationContext,
) -> Result<(), OAuthError> {
    validate_payload(payload, context)?;
    let extra = payload.extra_body.get_or_insert_with(Default::default);
    extra.retain(|name, _| !machine(name));
    extra.insert(
        "kiroMachineId".into(),
        serde_json::json!(context.machine_id()),
    );
    Ok(())
}
