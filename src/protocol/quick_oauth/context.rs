//! Credential-owned Amazon Quick IAM Identity Center registration and tenant routing metadata.
//! Only `client_secret` is secret material; expired dynamic client registrations stay readable
//! in saved route configuration and fail closed only when a refresh or send is attempted.

use super::{atom, OAuthError, RefreshGrant, DEFAULT_DATA_PLANE, DEFAULT_WS_HOST};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

// Commercial AWS IAM Identity Center OIDC regions allowlist.
const COMMERCIAL_REGIONS: &[&str] = &[
    "us-east-2",
    "us-east-1",
    "us-west-1",
    "us-west-2",
    "af-south-1",
    "ap-east-1",
    "ap-south-2",
    "ap-southeast-3",
    "ap-southeast-5",
    "ap-southeast-4",
    "ap-south-1",
    "ap-southeast-6",
    "ap-northeast-3",
    "ap-northeast-2",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-east-2",
    "ap-southeast-7",
    "ap-northeast-1",
    "ca-central-1",
    "ca-west-1",
    "eu-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-south-1",
    "eu-west-3",
    "eu-south-2",
    "eu-north-1",
    "eu-central-2",
    "il-central-1",
    "mx-central-1",
    "me-south-1",
    "me-central-1",
    "sa-east-1",
];

pub fn commercial_region(region: &str) -> bool {
    COMMERCIAL_REGIONS.contains(&region)
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationContext {
    pub(crate) idc_region: String,
    pub(crate) issuer_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tenant_url: Option<String>,
    pub(crate) ws_host: String,
    pub(crate) client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) client_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) client_secret_expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) actor_arn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) aws_account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) account_name: Option<String>,
}

impl AuthorizationContext {
    pub fn idc_region(&self) -> &str {
        &self.idc_region
    }

    pub fn issuer_url(&self) -> &str {
        &self.issuer_url
    }

    pub fn tenant_url(&self) -> Option<&str> {
        self.tenant_url.as_deref()
    }

    pub fn ws_host(&self) -> &str {
        &self.ws_host
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn client_secret(&self) -> Option<&str> {
        self.client_secret.as_deref()
    }

    pub fn actor_arn(&self) -> Option<&str> {
        self.actor_arn.as_deref()
    }

    pub fn aws_account_id(&self) -> Option<&str> {
        self.aws_account_id.as_deref()
    }

    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn account_name(&self) -> Option<&str> {
        self.account_name.as_deref()
    }

    pub(crate) fn take_client_secret(&mut self) -> Option<String> {
        self.client_secret.take()
    }

    pub fn oidc_origin_for_region(region: &str) -> Result<String, OAuthError> {
        if !commercial_region(region) {
            return Err(OAuthError::InvalidInput);
        }
        #[cfg(test)]
        if let Ok(origin) = super::TEST_OIDC_ORIGIN.try_with(|v| v.clone()) {
            return Ok(origin.trim_end_matches('/').to_owned());
        }
        Ok(format!("https://oidc.{region}.amazonaws.com"))
    }

    pub fn token_endpoint(&self) -> String {
        #[cfg(test)]
        if let Ok(origin) = super::TEST_OIDC_ORIGIN.try_with(|v| v.clone()) {
            return format!("{}/token", origin.trim_end_matches('/'));
        }
        format!("https://oidc.{}.amazonaws.com/token", self.idc_region)
    }

    pub fn validate_metadata(&self) -> Result<(), OAuthError> {
        if !commercial_region(&self.idc_region)
            || !valid_https_url(&self.issuer_url)
            || !valid_ws_host(&self.ws_host)
            || !atom(&self.client_id, 4096)
        {
            return Err(OAuthError::InvalidInput);
        }
        if let Some(tenant) = &self.tenant_url {
            if !valid_https_url(tenant) {
                return Err(OAuthError::InvalidInput);
            }
        }
        if let Some(exp) = self.client_secret_expires_at {
            if exp <= 0 || OffsetDateTime::from_unix_timestamp(exp).is_err() {
                return Err(OAuthError::InvalidInput);
            }
        }
        for optional in [
            self.actor_arn.as_deref(),
            self.aws_account_id.as_deref(),
            self.email.as_deref(),
            self.display_name.as_deref(),
            self.account_name.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if optional.trim().is_empty()
                || optional.len() > 512
                || optional.chars().any(char::is_control)
            {
                return Err(OAuthError::InvalidInput);
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), OAuthError> {
        self.validate_metadata()?;
        let secret = self
            .client_secret
            .as_deref()
            .filter(|s| atom(s, 16_384))
            .ok_or(OAuthError::InvalidInput)?;
        let _ = secret;
        Ok(())
    }

    pub(crate) fn refresh_grant(&self, refresh_token: &str) -> Result<RefreshGrant, OAuthError> {
        self.validate()?;
        if !super::valid_token(refresh_token) {
            return Err(OAuthError::InvalidInput);
        }
        if let Some(exp) = self.client_secret_expires_at {
            if OffsetDateTime::now_utc().unix_timestamp() >= exp {
                return Err(OAuthError::Expired);
            }
        }
        Ok(RefreshGrant {
            token_endpoint: self.token_endpoint(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone().ok_or(OAuthError::InvalidInput)?,
            refresh_token: refresh_token.to_owned(),
        })
    }
}

pub(crate) fn resolve_data_plane(tenant_url: Option<&str>) -> Result<(String, String), OAuthError> {
    #[cfg(test)]
    if let Ok(origin) = super::TEST_DATA_PLANE_ORIGIN.try_with(|v| v.clone()) {
        let parsed = url::Url::parse(&origin).map_err(|_| OAuthError::InvalidInput)?;
        let host = match parsed.port() {
            Some(port) => format!(
                "{}:{port}",
                parsed.host_str().ok_or(OAuthError::InvalidInput)?
            ),
            None => parsed
                .host_str()
                .ok_or(OAuthError::InvalidInput)?
                .to_owned(),
        };
        return Ok((origin.trim_end_matches('/').to_owned(), host));
    }
    if let Some(raw) = tenant_url.map(str::trim).filter(|s| !s.is_empty()) {
        if !valid_https_url(raw) {
            return Err(OAuthError::InvalidCallback);
        }
        let parsed = url::Url::parse(raw).map_err(|_| OAuthError::InvalidCallback)?;
        if let Some(host) = parsed.host_str() {
            if host.contains("dp.appintegrations") && valid_ws_host(host) {
                return Ok((format!("https://{host}"), host.to_owned()));
            }
        }
    }
    Ok((DEFAULT_DATA_PLANE.to_owned(), DEFAULT_WS_HOST.to_owned()))
}

pub(crate) fn valid_https_url(value: &str) -> bool {
    if !atom(value, 4096) {
        return false;
    }
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

pub(crate) fn valid_ws_host(host: &str) -> bool {
    #[cfg(test)]
    if host.starts_with("127.0.0.1:") || host == "127.0.0.1" {
        return true;
    }
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && (host == DEFAULT_WS_HOST
            || host.ends_with(".amazonaws.com")
            || host.ends_with(".aws.dev")
            || host.ends_with(".api.aws"))
}

impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickAuthorizationContext")
            .field("idc_region", &self.idc_region)
            .field("ws_host", &self.ws_host)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}
