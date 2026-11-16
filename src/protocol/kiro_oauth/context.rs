//! Credential-owned AWS registration and stable device identity; only client_secret is redacted.
//! Expired registrations remain readable evidence; only a live restored registration can send.
use super::{atom, ClientRegistration, Directory, OAuthError, Scope};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    BuilderId,
    IdentityCenter,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationContext {
    auth_method: AuthMethod,
    directory_region: String,
    start_url: String,
    client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    client_secret_expires_at: i64,
    scopes: Vec<Scope>,
    machine_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    runtime_target: Option<super::RuntimeTarget>,
}

impl AuthorizationContext {
    pub fn from_registration(
        registration: &ClientRegistration,
        machine_id: &str,
    ) -> Result<Self, OAuthError> {
        let context = Self {
            auth_method: if registration.directory() == &Directory::builder_id() {
                AuthMethod::BuilderId
            } else {
                AuthMethod::IdentityCenter
            },
            directory_region: registration.directory().region().into(),
            start_url: registration.directory().start_url().into(),
            client_id: registration.client_id().into(),
            client_secret: Some(registration.client_secret().into()),
            client_secret_expires_at: registration.expires_at().unix_timestamp(),
            scopes: registration.scopes().into(),
            machine_id: machine_id.into(),
            runtime_target: None,
        };
        context.validate()?;
        Ok(context)
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn client_secret(&self) -> Option<&str> {
        self.client_secret.as_deref()
    }
    pub fn machine_id(&self) -> &str {
        &self.machine_id
    }
    pub fn runtime_target(&self) -> Option<&super::RuntimeTarget> {
        self.runtime_target.as_ref()
    }
    pub(crate) fn bind_runtime(&mut self, target: super::RuntimeTarget) {
        self.runtime_target = Some(target);
    }
    pub fn token_endpoint(&self) -> String {
        format!("https://oidc.{}.amazonaws.com/token", self.directory_region)
    }
    pub(crate) fn take_client_secret(&mut self) -> Option<String> {
        self.client_secret.take()
    }
    fn metadata(&self) -> Result<(Directory, OffsetDateTime), OAuthError> {
        if let Some(target) = &self.runtime_target {
            target.validate()?;
        }
        let directory = match self.auth_method {
            AuthMethod::BuilderId => {
                let directory = Directory::builder_id();
                if self.directory_region != directory.region()
                    || self.start_url != directory.start_url()
                {
                    return Err(OAuthError::InvalidInput);
                }
                directory
            }
            AuthMethod::IdentityCenter => {
                Directory::identity_center(&self.directory_region, &self.start_url)?
            }
        };
        if !atom(&self.client_id, 4096)
            || self.scopes.is_empty()
            || self.scopes.len() > 5
            || self
                .scopes
                .iter()
                .enumerate()
                .any(|(i, scope)| self.scopes[..i].contains(scope))
            || self.machine_id.len() != 64
            || !self
                .machine_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.client_secret_expires_at <= 0
        {
            return Err(OAuthError::InvalidInput);
        }
        let expiry = OffsetDateTime::from_unix_timestamp(self.client_secret_expires_at)
            .map_err(|_| OAuthError::InvalidInput)?;
        Ok((directory, expiry))
    }
    pub fn validate_metadata(&self) -> Result<(), OAuthError> {
        self.metadata().map(|_| ())
    }
    fn checked_parts(&self) -> Result<(Directory, OffsetDateTime, &str), OAuthError> {
        let (directory, expiry) = self.metadata()?;
        let secret = self
            .client_secret
            .as_deref()
            .filter(|secret| atom(secret, 16_384))
            .ok_or(OAuthError::InvalidInput)?;
        Ok((directory, expiry, secret))
    }
    pub fn validate(&self) -> Result<(), OAuthError> {
        self.checked_parts().map(|_| ())
    }
    pub(crate) fn registration(&self) -> Result<ClientRegistration, OAuthError> {
        let (directory, expires_at, secret) = self.checked_parts()?;
        let remaining: std::time::Duration = (expires_at - OffsetDateTime::now_utc())
            .try_into()
            .map_err(|_| OAuthError::RegistrationExpired)?;
        let deadline = tokio::time::Instant::now()
            .checked_add(remaining)
            .ok_or(OAuthError::InvalidInput)?;
        let registration = ClientRegistration {
            directory,
            scopes: self.scopes.clone(),
            client_id: self.client_id.clone(),
            secret: secret.into(),
            expires_at,
            deadline,
        };
        registration.remaining()?;
        Ok(registration)
    }
}

impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KiroAuthorizationContext { registration: <redacted> }")
    }
}
