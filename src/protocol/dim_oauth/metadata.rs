//! Discovery can select paths only inside the already selected DIM authority, never a new audience.
use super::{reply, OAuthClient, OAuthError};
use serde_json::Value;

#[derive(Clone, PartialEq, Eq)]
pub struct Metadata {
    pub(super) client: OAuthClient,
    pub(super) authorization_endpoint: String,
    pub(super) token_endpoint: String,
    pub(super) userinfo_endpoint: String,
    pub(super) jwks_uri: String,
}
impl Metadata {
    pub fn parse(client: &OAuthClient, bytes: &[u8]) -> Result<Self, OAuthError> {
        let value = reply::object(bytes, 65_536)?;
        let issuer = value["issuer"]
            .as_str()
            .ok_or(OAuthError::InvalidResponse)?;
        if issuer.trim_end_matches('/') != client.site().issuer()
            || !declares(&value, "response_types_supported", "code")
            || !declares(&value, "code_challenge_methods_supported", "S256")
            || !declares(&value, "token_endpoint_auth_methods_supported", "none")
            || !declares(&value, "id_token_signing_alg_values_supported", "RS256")
        {
            return Err(OAuthError::InvalidMetadata);
        }
        let endpoint = |name: &str| {
            same_authority_endpoint(
                client,
                value[name].as_str().ok_or(OAuthError::InvalidMetadata)?,
            )
        };
        Ok(Self {
            client: client.clone(),
            authorization_endpoint: endpoint("authorization_endpoint")?,
            token_endpoint: endpoint("token_endpoint")?,
            userinfo_endpoint: endpoint("userinfo_endpoint")?,
            jwks_uri: endpoint("jwks_uri")?,
        })
    }
    pub fn client(&self) -> &OAuthClient {
        &self.client
    }
    pub fn token_endpoint(&self) -> &str {
        &self.token_endpoint
    }
    pub fn userinfo_endpoint(&self) -> &str {
        &self.userinfo_endpoint
    }
    pub fn jwks_uri(&self) -> &str {
        &self.jwks_uri
    }
}
fn declares(value: &Value, name: &str, expected: &str) -> bool {
    value[name].as_array().is_some_and(|items| {
        !items.is_empty()
            && items.len() <= 64
            && items.iter().all(Value::is_string)
            && items.iter().any(|item| item.as_str() == Some(expected))
    })
}
pub(super) fn same_authority_endpoint(
    client: &OAuthClient,
    value: &str,
) -> Result<String, OAuthError> {
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(OAuthError::InvalidMetadata);
    }
    let url = url::Url::parse(value).map_err(|_| OAuthError::InvalidMetadata)?;
    let issuer = url::Url::parse(client.site().issuer()).expect("fixed DIM site");
    if url.origin() != issuer.origin()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(OAuthError::InvalidMetadata);
    }
    Ok(url.into())
}
impl std::fmt::Debug for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimMetadata { authority: <validated> }")
    }
}
