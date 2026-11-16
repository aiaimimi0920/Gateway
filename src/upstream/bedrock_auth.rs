//! Bedrock material selection. Explicit modes never fall back to another identity.
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

#[path = "bedrock_target.rs"]
mod target;
pub(crate) use target::validate_model_id;

pub(super) struct SigningCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub region: String,
}

pub(super) enum Authentication {
    Legacy,
    Bearer(String),
    SigV4(SigningCredentials),
}

fn material_field(key: &str) -> Option<&'static str> {
    // Match the historical signing reader's case / '_' / '-' aliases in the wire filter too.
    let key = key
        .chars()
        .filter(|ch| !matches!(ch, '_' | '-'))
        .collect::<String>()
        .to_ascii_lowercase();
    match key.as_str() {
        "awsaccesskeyid" => Some("access"),
        "awssecretaccesskey" => Some("secret"),
        "awssessiontoken" => Some("session"),
        "awsregion" | "region" => Some("region"),
        _ => None,
    }
}

pub(crate) fn reserved_extra_key(key: &str) -> bool {
    material_field(key).is_some()
}

fn invalid() -> GatewayError {
    // Never interpolate supplied material, endpoint userinfo or headers into diagnostics.
    GatewayError::bad_request("Invalid or conflicting Bedrock authentication material")
        .with_code("bedrock_auth_invalid")
}

fn field(
    payload: &ProviderAccountPayload,
    expected: &str,
    explicit: bool,
) -> Result<Option<String>, GatewayError> {
    let mut found: Option<&str> = None;
    for (name, value) in payload.extra_body.iter().flatten() {
        if material_field(name) != Some(expected) {
            continue;
        }
        let value = value.as_str().ok_or_else(invalid)?;
        let value = if explicit { value } else { value.trim() };
        if !valid_secret(value) {
            return Err(invalid());
        }
        if found.is_some_and(|old| old != value) {
            return Err(invalid());
        }
        found = Some(value);
    }
    Ok(found.map(str::to_owned))
}

fn valid_secret(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32 * 1024
        && value.bytes().all(|byte| (33..=126).contains(&byte))
}

pub(crate) fn explicit_mode(payload: &ProviderAccountPayload) -> bool {
    crate::protocol::bedrock_converse::explicit_auth_mode(payload.auth_mode.as_deref())
}

pub(crate) fn validate_adapter(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    if explicit_mode(payload) && payload.canonical_adapter() != "bedrock_converse_compatible" {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn resolve(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Result<Authentication, GatewayError> {
    resolve_material(payload, Some(plan))
}

/// Compile only executable explicit accounts, using the same reader as final-send authentication.
pub(crate) fn validate_configuration(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    if explicit_mode(payload) {
        validate_adapter(payload)?;
        resolve_material(payload, None)?;
    }
    Ok(())
}

fn resolve_material(
    payload: &ProviderAccountPayload,
    plan: Option<&RequestPlan>,
) -> Result<Authentication, GatewayError> {
    let mode = payload
        .auth_mode
        .as_deref()
        .map(str::trim)
        .filter(|mode| !mode.is_empty());
    let explicit = explicit_mode(payload);
    if !matches!(
        mode,
        None | Some("bearer" | "bedrock_bearer" | "aws_sigv4" | "aws_sigv4_temporary")
    ) {
        return Err(invalid());
    }
    let access = field(payload, "access", explicit)?;
    let secret = field(payload, "secret", explicit)?;
    let session = field(payload, "session", explicit)?;
    let region = field(payload, "region", explicit)?;
    let has_iam = access.is_some() || secret.is_some() || session.is_some();
    if explicit {
        if payload.auth_token.is_some()
            || payload.session_auth.is_some()
            || payload
                .auth_header_name
                .as_deref()
                .is_some_and(|name| !name.eq_ignore_ascii_case("authorization"))
        {
            return Err(invalid());
        }
        target::validate(payload, plan, region.as_deref().ok_or_else(invalid)?)?;
    }
    if mode == Some("bedrock_bearer") {
        if has_iam || !valid_secret(&payload.api_key) {
            return Err(invalid());
        }
        return Ok(Authentication::Bearer(payload.api_key.clone()));
    }
    if !has_iam && !explicit {
        return Ok(Authentication::Legacy);
    }
    if explicit && !payload.api_key.is_empty() {
        return Err(invalid());
    }
    if mode == Some("aws_sigv4_temporary") && session.is_none() {
        return Err(invalid());
    }
    let region = region
        .or_else(|| legacy_region(&payload.base_url))
        .ok_or_else(invalid)?;
    Ok(Authentication::SigV4(SigningCredentials {
        access_key_id: access.ok_or_else(invalid)?,
        secret_access_key: secret.ok_or_else(invalid)?,
        session_token: session,
        region,
    }))
}

fn legacy_region(base: &str) -> Option<String> {
    // Preserve operator-configured historical endpoints. New material modes never use this fallback.
    let url = url::Url::parse(base).ok()?;
    let host = url.host_str()?;
    [
        "bedrock-runtime.",
        "bedrock-runtime-fips.",
        "bedrock.",
        "bedrock-fips.",
    ]
    .into_iter()
    .find_map(|prefix| {
        host.strip_prefix(prefix)?
            .split('.')
            .next()
            .filter(|region| !region.is_empty())
            .map(str::to_owned)
    })
}
