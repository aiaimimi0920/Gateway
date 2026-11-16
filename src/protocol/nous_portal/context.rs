//! JWT claims are unverified eligibility hints; they never authenticate management or merge accounts.
use super::{atom, wire, OAuthClient, OAuthError, INFERENCE_SCOPE};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceTarget {
    Subscription,
    Welcome,
}
impl InferenceTarget {
    pub fn base_url(self) -> &'static str {
        match self {
            Self::Subscription => "https://inference-api.nousresearch.com/v1",
            Self::Welcome => "https://welcome-api.nousresearch.com/v1",
        }
    }
    pub fn from_base_url(value: &str) -> Result<Self, OAuthError> {
        match value {
            "https://inference-api.nousresearch.com/v1"
            | "https://inference-api.nousresearch.com/v1/" => Ok(Self::Subscription),
            "https://welcome-api.nousresearch.com/v1"
            | "https://welcome-api.nousresearch.com/v1/" => Ok(Self::Welcome),
            _ => Err(OAuthError::UnsafeInferenceTarget),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationContext {
    client_id: String,
    inference: InferenceTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subject_hint: Option<String>,
    access_digest: String,
    scope: String,
    expires_at_unix: u64,
}
impl AuthorizationContext {
    pub fn validate(&self) -> Result<(), OAuthError> {
        OAuthClient::new(&self.client_id)?;
        if self.scope != INFERENCE_SCOPE
            || self.access_digest.len() != 64
            || !self
                .access_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.subject_hint.as_deref().is_some_and(|s| !atom(s, 512))
            || self.expires_at_unix == 0
            || i64::try_from(self.expires_at_unix)
                .ok()
                .and_then(|v| time::OffsetDateTime::from_unix_timestamp(v).ok())
                .is_none()
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(())
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn inference_target(&self) -> InferenceTarget {
        self.inference
    }
    pub fn subject_hint(&self) -> Option<&str> {
        self.subject_hint.as_deref()
    }
    pub fn expires_at_unix(&self) -> u64 {
        self.expires_at_unix
    }
    pub(crate) fn matches_expiry(&self, value: &str) -> bool {
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
            .is_ok_and(|expiry| {
                time::OffsetDateTime::from_unix_timestamp(self.expires_at_unix as i64)
                    .is_ok_and(|expected| expiry == expected)
            })
    }
    pub fn validate_access(&self, access: &str) -> Result<(), OAuthError> {
        self.validate_binding(access, &OAuthClient::new(&self.client_id)?)?;
        let claims = claims(access)?;
        check_not_before(&claims, super::unix_now())?;
        if self.expires_at_unix <= super::unix_now() {
            return Err(OAuthError::Expired);
        }
        Ok(())
    }
    pub(crate) fn validate_binding(
        &self,
        access: &str,
        client: &OAuthClient,
    ) -> Result<(), OAuthError> {
        self.validate()?;
        let claims = claims(access)?;
        if let Some(expiry) = claims.get("exp") {
            let expiry = expiry.as_u64().ok_or(OAuthError::InvalidAccessJwt)?;
            if self.expires_at_unix > expiry {
                return Err(OAuthError::InvalidAccessJwt);
            }
        }
        if self.client_id != client.client_id()
            || digest(access) != self.access_digest
            || subject(&claims)? != self.subject_hint
        {
            return Err(OAuthError::SubjectHintMismatch);
        }
        Ok(())
    }
    /// Continuity is the exact consumed refresh chain, not equality of two unknown JWT subjects.
    pub fn permits_rotation(&self, next: &Self) -> bool {
        self.validate().is_ok()
            && next.validate().is_ok()
            && self.client_id == next.client_id
            && self
                .subject_hint
                .as_ref()
                .is_none_or(|old| next.subject_hint.as_ref() == Some(old))
    }
    pub fn same_known_subject_hint(&self, other: &Self) -> bool {
        self.client_id == other.client_id
            && self
                .subject_hint
                .as_ref()
                .is_some_and(|subject| other.subject_hint.as_ref() == Some(subject))
    }
}

pub struct AuthorizedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_unix: u64,
    pub expires_in: u64,
    pub context: AuthorizationContext,
}

pub(super) fn interpret(
    response: &Value,
    received_at: u64,
    client: &OAuthClient,
    previous: Option<&AuthorizationContext>,
) -> Result<AuthorizedTokens, OAuthError> {
    if response.get("error").is_some()
        || response.get("token_type").is_some_and(|value| {
            !value
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("Bearer"))
        })
    {
        return Err(OAuthError::InvalidResponse);
    }
    let access = response
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or(OAuthError::InvalidAccessJwt)?;
    let claims = claims(access)?;
    check_not_before(&claims, received_at)?;
    let mut scopes = Vec::new();
    for scope in [
        response.get("scope"),
        claims.get("scope"),
        claims.get("scp"),
    ]
    .into_iter()
    .flatten()
    {
        collect_scopes(scope, &mut scopes)?;
    }
    if !scopes.iter().any(|scope| scope == INFERENCE_SCOPE) {
        return Err(OAuthError::ScopeMismatch);
    }
    let jwt_expiry = claims
        .get("exp")
        .map(|v| v.as_u64().ok_or(OAuthError::InvalidAccessJwt))
        .transpose()?;
    let response_expiry = response
        .get("expires_in")
        .map(|value| {
            let lifetime = value
                .as_u64()
                .filter(|n| *n > 0 && *n <= 100 * 365 * 86400)
                .ok_or(OAuthError::InvalidResponse)?;
            received_at
                .checked_add(lifetime)
                .ok_or(OAuthError::InvalidResponse)
        })
        .transpose()?;
    let expiry = match (jwt_expiry, response_expiry) {
        (Some(jwt), Some(response)) => jwt.min(response),
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return Err(OAuthError::InvalidResponse),
    };
    if expiry <= received_at
        || i64::try_from(expiry)
            .ok()
            .and_then(|v| time::OffsetDateTime::from_unix_timestamp(v).ok())
            .is_none()
    {
        return Err(OAuthError::Expired);
    }
    let inference = match response.get("inference_base_url") {
        Some(value) => InferenceTarget::from_base_url(
            value.as_str().ok_or(OAuthError::UnsafeInferenceTarget)?,
        )?,
        None => previous
            .map(|p| p.inference)
            .unwrap_or(InferenceTarget::Subscription),
    };
    let context = AuthorizationContext {
        client_id: client.client_id().into(),
        inference,
        subject_hint: subject(&claims)?,
        access_digest: digest(access),
        scope: INFERENCE_SCOPE.into(),
        expires_at_unix: expiry,
    };
    context.validate()?;
    if previous.is_some_and(|old| !old.permits_rotation(&context)) {
        return Err(OAuthError::SubjectHintMismatch);
    }
    Ok(AuthorizedTokens {
        access_token: access.into(),
        refresh_token: response["refresh_token"]
            .as_str()
            .ok_or(OAuthError::MissingSuccessor)?
            .into(),
        expires_at_unix: expiry,
        expires_in: expiry - received_at,
        context,
    })
}

pub(crate) fn subject_hint(access: &str) -> Result<Option<String>, OAuthError> {
    subject(&claims(access)?)
}
fn subject(claims: &Value) -> Result<Option<String>, OAuthError> {
    claims
        .get("sub")
        .map(|v| {
            v.as_str()
                .filter(|s| atom(s, 512))
                .map(str::to_owned)
                .ok_or(OAuthError::InvalidAccessJwt)
        })
        .transpose()
}
fn claims(access: &str) -> Result<Value, OAuthError> {
    if !atom(access, 32_768) {
        return Err(OAuthError::InvalidAccessJwt);
    }
    let parts: Vec<_> = access.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return Err(OAuthError::InvalidAccessJwt);
    }
    let decode = |part| {
        URL_SAFE_NO_PAD
            .decode(part)
            .map_err(|_| OAuthError::InvalidAccessJwt)
    };
    let header = wire::object(&decode(parts[0])?).map_err(|_| OAuthError::InvalidAccessJwt)?;
    if !header
        .get("alg")
        .and_then(Value::as_str)
        .is_some_and(|s| atom(s, 128) && s != "none")
    {
        return Err(OAuthError::InvalidAccessJwt);
    }
    decode(parts[2])?;
    wire::object(&decode(parts[1])?).map_err(|_| OAuthError::InvalidAccessJwt)
}
fn check_not_before(claims: &Value, at: u64) -> Result<(), OAuthError> {
    if let Some(value) = claims.get("nbf") {
        if value.as_u64().is_none_or(|nbf| nbf > at) {
            return Err(OAuthError::InvalidAccessJwt);
        }
    }
    Ok(())
}
fn collect_scopes(value: &Value, scopes: &mut Vec<String>) -> Result<(), OAuthError> {
    match value {
        Value::String(value)
            if value.len() <= 4096 && value.bytes().all(|b| (0x20..=0x7e).contains(&b)) =>
        {
            scopes.extend(
                value
                    .split([' ', ','])
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
        Value::Array(values) if values.len() <= 64 => {
            for value in values {
                if !value.is_string() {
                    return Err(OAuthError::InvalidResponse);
                }
                collect_scopes(value, scopes)?;
            }
        }
        _ => return Err(OAuthError::InvalidResponse),
    }
    Ok(())
}
fn digest(access: &str) -> String {
    hex::encode(Sha256::digest(access.as_bytes()))
}
impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "NousAuthorizationContext { assurance: unverified_jwt_hint, identity: <redacted> }",
        )
    }
}
impl std::fmt::Debug for AuthorizedTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NousAuthorizedTokens { material: <redacted>, signature_verified: false }")
    }
}
