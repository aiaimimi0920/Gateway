//! Immutable business authority: no URL inferred from login, no arbitrary target or GUI-derived client.
use super::{atom, CatalogError};
use rquest::header::HeaderValue;

/// These origins are publicly configured/fallback business deployments, not a release-binary attestation.
/// Choosing one is explicit; login at freebuff.com does not authorize an automatic cross-host send.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum BusinessOrigin {
    #[serde(rename = "https://codebuff.com")]
    Codebuff,
    #[serde(rename = "https://www.codebuff.com")]
    CodebuffWww,
}
impl BusinessOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codebuff => "https://codebuff.com",
            Self::CodebuffWww => "https://www.codebuff.com",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientSurface {
    Cli,
    Desktop,
}
impl ClientSurface {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Desktop => "desktop",
        }
    }
}
pub(super) struct Binding {
    pub origin: BusinessOrigin,
    pub client: ClientSurface,
    pub authorization: HeaderValue,
    pub scope: [u8; 32],
}
impl Binding {
    pub fn new(
        origin: BusinessOrigin,
        token: &str,
        client: ClientSurface,
    ) -> Result<Self, CatalogError> {
        if !atom(token, 16_384) {
            return Err(CatalogError::InvalidInput);
        }
        let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| CatalogError::InvalidInput)?;
        authorization.set_sensitive(true);
        let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
        for value in [origin.as_str(), client.as_str(), token] {
            digest.update(&(value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
        let scope = digest.finish().as_ref().try_into().expect("SHA-256 size");
        Ok(Self {
            origin,
            client,
            authorization,
            scope,
        })
    }
    pub fn transport_origin(&self) -> String {
        let origin = self.origin.as_str().to_owned();
        #[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
        let origin = super::TEST_ORIGIN
            .try_with(Clone::clone)
            .map(|origin| {
                let url = url::Url::parse(&origin).unwrap();
                assert_eq!(url.scheme(), "http");
                assert_eq!(url.host_str(), Some("127.0.0.1"));
                assert!(url.port().is_some() && url.path() == "/");
                assert!(url.query().is_none() && url.fragment().is_none());
                assert!(url.username().is_empty() && url.password().is_none());
                origin
            })
            .unwrap_or(origin);
        origin
    }
}
