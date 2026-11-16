//! Access-token claims are unverified hints. They never authorize management or merge quota pools.
use super::{material::parse_reply, valid_atom, OAuthError, Principal};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::Value;

pub struct PrincipalHint {
    kind: Option<String>,
    id: Option<String>,
}
impl PrincipalHint {
    pub fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    pub(super) fn read(token: &str) -> Result<Option<Self>, OAuthError> {
        if token.len() > 32_768 {
            return Err(OAuthError::InvalidResponse);
        }
        let mut parts = token.split('.');
        let header = parts.next().unwrap_or_default();
        let (Some(payload), Some(signature)) = (parts.next(), parts.next()) else {
            return Ok(None);
        };
        if parts.next().is_some() {
            return Ok(None);
        }
        // RFC 6749 leaves access tokens opaque. Dots alone do not make one a JWT.
        let Ok(header_bytes) = URL_SAFE_NO_PAD.decode(header) else {
            return Ok(None);
        };
        let Ok(header): Result<Value, _> = serde_json::from_slice(&header_bytes) else {
            return Ok(None);
        };
        if !header.get("alg").is_some_and(Value::is_string) {
            return Ok(None);
        }
        let header = parse_reply(&header_bytes)?;
        if !header["alg"].as_str().is_some_and(|s| valid_atom(s, 128))
            || payload.is_empty()
            || signature.is_empty()
        {
            return Err(OAuthError::InvalidResponse);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| OAuthError::InvalidResponse)?;
        let body = parse_reply(&bytes)?;
        let kind = field(&body, "principal_type", "principalType", 128)?;
        let id = field(&body, "principal_id", "principalId", 512)?;
        Ok(if kind.is_some() || id.is_some() {
            Some(Self { kind, id })
        } else {
            None
        })
    }
    pub(super) fn is_team(&self) -> bool {
        self.kind() == Some("Team") && self.id.is_some()
    }
}
fn field(
    body: &Value,
    canonical: &str,
    alias: &str,
    limit: usize,
) -> Result<Option<String>, OAuthError> {
    if body.get(canonical).is_some() && body.get(alias).is_some() {
        return Err(OAuthError::InvalidResponse);
    }
    body.get(canonical)
        .or_else(|| body.get(alias))
        .map(|v| {
            v.as_str()
                .filter(|s| valid_atom(s, limit))
                .map(str::to_owned)
                .ok_or(OAuthError::InvalidResponse)
        })
        .transpose()
}
impl std::fmt::Debug for PrincipalHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UnverifiedGrokPrincipal { hint: <redacted> }")
    }
}

/// Separate from consent pre-selection. A fail-fast constraint is not issuer signature proof.
#[derive(Clone, PartialEq, Eq)]
pub struct PrincipalConstraint {
    kind: Option<String>,
    id: String,
}
impl PrincipalConstraint {
    pub fn for_id(id: &str) -> Result<Self, OAuthError> {
        if !valid_atom(id, 512) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            kind: None,
            id: id.into(),
        })
    }
    pub fn for_principal(principal: Principal) -> Self {
        Self {
            kind: Some(principal.kind),
            id: principal.id,
        }
    }
    pub(super) fn check(&self, actual: Option<&PrincipalHint>) -> Result<(), OAuthError> {
        if actual.is_some_and(|p| {
            p.id() == Some(self.id.as_str())
                && self
                    .kind
                    .as_deref()
                    .is_none_or(|kind| p.kind() == Some(kind))
        }) {
            Ok(())
        } else {
            Err(OAuthError::PrincipalMismatch)
        }
    }
}
impl std::fmt::Debug for PrincipalConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokPrincipalConstraint { expected: <redacted> }")
    }
}
