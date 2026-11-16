//! Metadata has one send+decoded-read budget, no redirects, retries or inherited credentials.
use super::{metadata::same_authority_endpoint, site::atom, Metadata, OAuthClient, OAuthError};
use futures::StreamExt;
use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(15);
const JSON_LIMIT: usize = 65_536;
const CATALOG_LIMIT: usize = 2 * 1024 * 1024;

impl OAuthClient {
    pub async fn discover(&self, http: &rquest::Client) -> Result<Metadata, OAuthError> {
        let endpoint = format!("{}/.well-known/openid-configuration", self.site().issuer());
        let bytes = get(http, self, &endpoint, None, JSON_LIMIT).await?;
        Metadata::parse(self, &bytes)
    }
    pub async fn model_catalog(
        &self,
        http: &rquest::Client,
        access_token: &str,
    ) -> Result<Vec<super::DeclaredModel>, OAuthError> {
        if !atom(access_token, 32_768) {
            return Err(OAuthError::InvalidInput);
        }
        let mut endpoint = url::Url::parse(&format!("{}/v1/models", self.site().issuer()))
            .expect("fixed DIM site");
        endpoint.query_pairs_mut().append_pair("type", "dim");
        let bytes = get(
            http,
            self,
            endpoint.as_str(),
            Some(access_token),
            CATALOG_LIMIT,
        )
        .await?;
        super::parse_catalog(&bytes)
    }
}

pub(super) async fn get(
    http: &rquest::Client,
    client: &OAuthClient,
    endpoint: &str,
    bearer: Option<&str>,
    max: usize,
) -> Result<Vec<u8>, OAuthError> {
    let mut checked = url::Url::parse(endpoint).map_err(|_| OAuthError::InvalidMetadata)?;
    let query = checked.query().map(str::to_owned);
    checked.set_query(None);
    same_authority_endpoint(client, checked.as_str())?;
    if query.is_some() && (checked.path() != "/v1/models" || query.as_deref() != Some("type=dim")) {
        return Err(OAuthError::InvalidMetadata);
    }
    let endpoint = endpoint.to_owned();
    #[cfg(test)]
    let endpoint = test_target(&endpoint);
    let budget = DEADLINE;
    #[cfg(test)]
    let budget = TEST_BUDGET
        .try_with(|value| budget.min(*value))
        .unwrap_or(budget);
    tokio::time::timeout(budget, async {
        let mut request = http
            .get(endpoint)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header("accept", "application/json")
            .header("user-agent", "NeuroGateway/0.1 dim-oauth")
            .header("x-title", "Neuro Gateway");
        if let Some(token) = bearer {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.map_err(transport_error)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(OAuthError::RedirectRejected);
        }
        // Status alone is authoritative here; never wait on an untrusted error body to retire 401.
        if !status.is_success() {
            return Err(if status.as_u16() == 401 {
                OAuthError::ReauthorizationRequired
            } else {
                OAuthError::Rejected
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > max as u64)
        {
            return Err(OAuthError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(transport_error)?;
            if bytes.len().saturating_add(chunk.len()) > max {
                return Err(OAuthError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| OAuthError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    })
    .await
    .map_err(|_| OAuthError::Timeout)?
}
pub(super) fn transport_error(error: rquest::Error) -> OAuthError {
    if error.is_timeout() {
        OAuthError::Timeout
    } else {
        OAuthError::Transport
    }
}

#[cfg(test)]
tokio::task_local! { pub(super) static TEST_ORIGIN: String; }
#[cfg(test)]
tokio::task_local! { pub(super) static TEST_BUDGET: Duration; }
#[cfg(test)]
pub(super) fn test_target(endpoint: &str) -> String {
    TEST_ORIGIN
        .try_with(|origin| {
            let peer = url::Url::parse(origin).unwrap();
            assert_eq!(peer.scheme(), "http");
            assert_eq!(peer.host_str(), Some("127.0.0.1"));
            assert!(peer.port().is_some());
            let mut endpoint = url::Url::parse(endpoint).unwrap();
            endpoint.set_scheme("http").unwrap();
            endpoint.set_host(Some("127.0.0.1")).unwrap();
            endpoint.set_port(peer.port()).unwrap();
            endpoint.into()
        })
        .unwrap_or_else(|_| endpoint.into())
}
