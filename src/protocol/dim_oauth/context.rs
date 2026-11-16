//! Credential-only authority snapshot. Tokens stay in existing secret fields, never this context.
use super::{
    grant::{parse_scopes, unix_now, Tokens},
    metadata::same_authority_endpoint,
    site::atom,
    GrantReceipt, IdentityAssurance, OAuthClient, OAuthError, Site,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationContext {
    site: Site,
    client_id: String,
    requested_scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    granted_scopes: Option<Vec<String>>,
    token_endpoint: String,
    userinfo_endpoint: String,
    jwks_uri: String,
    subject: String,
    user_id: String,
    assurance: IdentityAssurance,
    nonce: String,
}
impl AuthorizationContext {
    pub fn from_receipt(receipt: &GrantReceipt) -> Result<Self, OAuthError> {
        if receipt.is_expired() {
            return Err(OAuthError::Expired);
        }
        let metadata = &receipt.metadata;
        let context = Self {
            site: metadata.client().site(),
            client_id: metadata.client().client_id().into(),
            requested_scopes: metadata.client().scopes().to_vec(),
            granted_scopes: receipt.granted_scopes.clone(),
            token_endpoint: metadata.token_endpoint().into(),
            userinfo_endpoint: metadata.userinfo_endpoint().into(),
            jwks_uri: metadata.jwks_uri().into(),
            subject: receipt.subject.clone(),
            user_id: receipt.user_id.clone(),
            assurance: receipt.assurance,
            nonce: receipt.nonce.clone(),
        };
        context.validate()?;
        Ok(context)
    }
    pub fn validate(&self) -> Result<(), OAuthError> {
        let scopes: Vec<_> = self.requested_scopes.iter().map(String::as_str).collect();
        let client = OAuthClient::new(self.site, &self.client_id, &scopes)?;
        same_authority_endpoint(&client, &self.token_endpoint)?;
        same_authority_endpoint(&client, &self.userinfo_endpoint)?;
        same_authority_endpoint(&client, &self.jwks_uri)?;
        if !atom(&self.nonce, 256)
            || !atom(&self.subject, 255)
            || self.user_id.is_empty()
            || self.user_id.len() > 512
            || self.user_id.chars().any(char::is_control)
        {
            return Err(OAuthError::InvalidInput);
        }
        if let Some(granted) = &self.granted_scopes {
            let parsed = parse_scopes(&granted.join(" "))?;
            if parsed != *granted || granted.iter().any(|s| !self.requested_scopes.contains(s)) {
                return Err(OAuthError::ScopeMismatch);
            }
        }
        Ok(())
    }
    pub fn site(&self) -> Site {
        self.site
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
    pub fn token_endpoint(&self) -> &str {
        &self.token_endpoint
    }
    pub fn identity_assurance(&self) -> IdentityAssurance {
        self.assurance
    }
    pub async fn model_catalog(
        &self,
        http: &rquest::Client,
        access: &str,
    ) -> Result<super::ModelCatalog, OAuthError> {
        self.validate()?;
        let scopes: Vec<_> = self.requested_scopes.iter().map(String::as_str).collect();
        let client = OAuthClient::new(self.site, &self.client_id, &scopes)?;
        let models = client.model_catalog(http, access).await?;
        super::ModelCatalog::new(self, models).map_err(|_| OAuthError::InvalidResponse)
    }
    pub fn refresh_grant(&self, refresh_token: &str) -> Result<RefreshGrant, OAuthError> {
        self.validate()?;
        if !atom(refresh_token, 32_768) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(RefreshGrant {
            context: self.clone(),
            refresh_token: refresh_token.into(),
        })
    }
    pub fn permits_rotation(&self, next: &Self) -> bool {
        self.validate().is_ok()
            && next.validate().is_ok()
            && self.site == next.site
            && self.client_id == next.client_id
            && self.requested_scopes == next.requested_scopes
            && self.subject == next.subject
            && self.user_id == next.user_id
            && self.assurance == next.assurance
            && self.nonce == next.nonce
            && self.token_endpoint == next.token_endpoint
            && self.userinfo_endpoint == next.userinfo_endpoint
            && self.jwks_uri == next.jwks_uri
            && match (&self.granted_scopes, &next.granted_scopes) {
                (Some(old), Some(new)) => new.iter().all(|s| old.contains(s)),
                (Some(_), None) => false,
                (None, _) => true,
            }
    }
    fn refreshed(&self, tokens: &Tokens) -> Result<Self, OAuthError> {
        let mut next = self.clone();
        if let Some(scope) = &tokens.scope {
            let scopes = parse_scopes(scope)?;
            let ceiling = self
                .granted_scopes
                .as_ref()
                .unwrap_or(&self.requested_scopes);
            if scopes.iter().any(|s| !ceiling.contains(s)) {
                return Err(OAuthError::ScopeMismatch);
            }
            next.granted_scopes = Some(scopes);
        }
        next.validate()?;
        Ok(next)
    }
}

/// The durable scheduler must reserve this exchange and persist a successor before another send.
pub struct RefreshGrant {
    context: AuthorizationContext,
    refresh_token: String,
}
pub struct RefreshedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_unix: u64,
    pub expires_in: u64,
    pub context: AuthorizationContext,
}
impl RefreshGrant {
    pub async fn execute(self, http: &rquest::Client) -> Result<RefreshedTokens, OAuthError> {
        let budget = std::time::Duration::from_secs(15);
        #[cfg(test)]
        let budget = super::transport::TEST_BUDGET
            .try_with(|value| budget.min(*value))
            .unwrap_or(budget);
        // Token delivery and optional JWKS validation share the scheduler's one exchange budget.
        tokio::time::timeout(budget, self.exchange(http))
            .await
            .map_err(|_| OAuthError::Timeout)?
    }
    async fn exchange(self, http: &rquest::Client) -> Result<RefreshedTokens, OAuthError> {
        let client = OAuthClient::new(
            self.context.site,
            &self.context.client_id,
            &self
                .context
                .requested_scopes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )?;
        // Persisted endpoints are pinned to the accepted grant, not overwritten by current defaults.
        let tokens = super::token_transport::post(
            http,
            &self.context.token_endpoint,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &self.refresh_token),
                ("client_id", &self.context.client_id),
            ],
            false,
        )
        .await?;
        // No new nonce is sent. If an ID token carries one, OIDC 12.2 requires the original nonce.
        if let Some(token) = &tokens.id_token {
            let keys =
                super::transport::get(http, &client, &self.context.jwks_uri, None, 65_536).await?;
            let identity = super::identity::verify_refresh(
                &client,
                token,
                &tokens.access_token,
                &self.context.nonce,
                &keys,
                unix_now()?,
            )?;
            if identity.subject() != self.context.subject {
                return Err(OAuthError::IdentityMismatch);
            }
        }
        let context = self.context.refreshed(&tokens)?;
        if tokens.expires_at_unix <= unix_now()? {
            return Err(OAuthError::Expired);
        }
        Ok(RefreshedTokens {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token.unwrap_or(self.refresh_token),
            expires_at_unix: tokens.expires_at_unix,
            expires_in: tokens.expires_in,
            context,
        })
    }
}
impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimAuthorizationContext { authority: <redacted> }")
    }
}
impl std::fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimRefreshGrant { material: <redacted> }")
    }
}
impl std::fmt::Debug for RefreshedTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimRefreshedTokens { material: <redacted> }")
    }
}
