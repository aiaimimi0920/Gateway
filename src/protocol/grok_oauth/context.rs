//! Non-secret grant snapshot, not a receipt or verified identity; never merged from provider defaults.
use super::{
    valid_atom, GrantReceipt, OAuthClient, OAuthError, Principal, PrincipalHint, UnverifiedTokens,
    ISSUER,
};
use serde::{Deserialize, Serialize};

pub const TOKEN_ENDPOINT: &str = "https://auth.x.ai/oauth2/token";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationContext {
    issuer: String,
    client_id: String,
    requested_scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    granted_scopes: Option<Vec<String>>,
    /// Consent pre-selection is recorded separately from what the endpoint actually returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    requested_principal: Option<PrincipalSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    principal: Option<PrincipalSnapshot>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrincipalSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

impl AuthorizationContext {
    pub fn from_receipt(receipt: &GrantReceipt) -> Result<Self, OAuthError> {
        if receipt.is_expired() {
            return Err(OAuthError::Expired);
        }
        let client = receipt.client();
        let context = Self {
            issuer: ISSUER.into(),
            client_id: client.client_id.clone(),
            requested_scopes: client.scope.split(' ').map(str::to_owned).collect(),
            granted_scopes: receipt
                .tokens()
                .scope
                .as_deref()
                .map(parse_scopes)
                .transpose()?,
            requested_principal: client.principal().map(|p| PrincipalSnapshot {
                kind: Some(p.kind().into()),
                id: Some(p.id().into()),
            }),
            principal: receipt.principal_hint().map(|p| PrincipalSnapshot {
                kind: p.kind().map(str::to_owned),
                id: p.id().map(str::to_owned),
            }),
        };
        context.validate()?;
        Ok(context)
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn validate(&self) -> Result<(), OAuthError> {
        self.client().map(|_| ())
    }
    pub(crate) fn client(&self) -> Result<OAuthClient, OAuthError> {
        if self.issuer != ISSUER {
            return Err(OAuthError::InvalidInput);
        }
        let scopes: Vec<_> = self.requested_scopes.iter().map(String::as_str).collect();
        let mut client = OAuthClient::new(&self.client_id, &scopes)?;
        if let Some(granted) = &self.granted_scopes {
            validate_scopes(granted)?;
            if granted.iter().any(|s| !self.requested_scopes.contains(s)) {
                return Err(OAuthError::InvalidInput);
            }
        }
        for principal in [&self.requested_principal, &self.principal]
            .into_iter()
            .flatten()
        {
            if (principal.kind.is_none() && principal.id.is_none())
                || principal
                    .kind
                    .as_deref()
                    .is_some_and(|s| !valid_atom(s, 128))
                || principal.id.as_deref().is_some_and(|s| !valid_atom(s, 512))
            {
                return Err(OAuthError::InvalidInput);
            }
        }
        // Only the actual complete hint is sent on refresh; a consent pre-selection is not a pin.
        if let Some(principal) = &self.principal {
            if let (Some(kind), Some(id)) = (&principal.kind, &principal.id) {
                client = client.with_principal(Principal::new(kind, id)?);
            }
        }
        Ok(client)
    }
    pub(crate) fn refreshed(&self, tokens: &UnverifiedTokens) -> Result<Self, OAuthError> {
        let mut next = self.clone();
        let hint = PrincipalHint::read(&tokens.access_token)?;
        if let Some(hint) = hint {
            let conflicts = self.principal.as_ref().is_some_and(|p| {
                hint.kind()
                    .is_some_and(|kind| p.kind.as_deref().is_some_and(|old| kind != old))
                    || hint
                        .id()
                        .is_some_and(|id| p.id.as_deref().is_some_and(|old| id != old))
            });
            if conflicts {
                return Err(OAuthError::PrincipalMismatch);
            }
            let old = self.principal.as_ref();
            next.principal = Some(PrincipalSnapshot {
                kind: hint
                    .kind()
                    .or_else(|| old.and_then(|p| p.kind.as_deref()))
                    .map(str::to_owned),
                id: hint
                    .id()
                    .or_else(|| old.and_then(|p| p.id.as_deref()))
                    .map(str::to_owned),
            });
        }
        // Opaque tokens remain valid endpoint-delivered material, not newly verified principal claims.
        if let Some(scope) = &tokens.scope {
            let returned = parse_scopes(scope)?;
            let allowed = self
                .granted_scopes
                .as_ref()
                .unwrap_or(&self.requested_scopes);
            if returned.iter().any(|s| !allowed.contains(s)) {
                return Err(OAuthError::ReauthorizationRequired);
            }
            // Persist observed narrowing with token rotation; it becomes the next refresh ceiling.
            next.granted_scopes = Some(returned);
        }
        next.validate()?;
        Ok(next)
    }
    /// Reuse the exchange rules when validating an archived, already accepted rotation.
    pub(crate) fn permits_rotation(&self, next: &Self, access_token: &str) -> bool {
        self.refreshed(&UnverifiedTokens {
            access_token: access_token.into(),
            refresh_token: None,
            id_token: None,
            expires_in: None,
            scope: next.granted_scopes.as_ref().map(|scopes| scopes.join(" ")),
        })
        .is_ok_and(|expected| expected == *next)
    }
}

fn parse_scopes(scope: &str) -> Result<Vec<String>, OAuthError> {
    let scopes: Vec<_> = scope.split(' ').map(str::to_owned).collect();
    validate_scopes(&scopes)?;
    Ok(scopes)
}
fn validate_scopes(scopes: &[String]) -> Result<(), OAuthError> {
    let references: Vec<_> = scopes.iter().map(String::as_str).collect();
    OAuthClient::new("scope-validation", &references)?;
    Ok(())
}
impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokAuthorizationContext { authorization: <redacted> }")
    }
}
