//! The client identity is scoped to one XSUAA token exchange, never to inference or discovery.
use super::{failure, GatewayError};

pub(super) const MAX_TOKEN_BYTES: usize = 65_536;
const FAILED: &str = "sap_aicore_token_failed";

pub(super) fn identity(certificate: &str, key: &str) -> Result<reqwest::Identity, GatewayError> {
    let certificate = certificate.trim();
    let key = key.trim();
    // Keep each field in its own format and reject ambiguous bundles with multiple private keys.
    if !certificate.starts_with("-----BEGIN CERTIFICATE-----")
        || certificate.contains("PRIVATE KEY-----")
        || key.matches("-----BEGIN ").count() != 1
        || ![
            "-----BEGIN PRIVATE KEY-----",
            "-----BEGIN RSA PRIVATE KEY-----",
        ]
        .iter()
        .any(|prefix| key.starts_with(prefix))
    {
        return Err(failure(FAILED));
    }
    reqwest::Identity::from_pem(format!("{certificate}\n{key}\n").as_bytes())
        .map_err(|_| failure(FAILED))
}

pub(super) async fn exchange(
    url: &str,
    client_id: &str,
    certificate: &str,
    key: &str,
    fixture: Option<&super::fixture::Transport>,
) -> Result<Vec<u8>, GatewayError> {
    let builder = reqwest::Client::builder()
        .use_rustls_tls()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .identity(identity(certificate, key)?);
    let url = format!("{url}/oauth/token");
    let (builder, url) = match fixture {
        Some(fixture) => fixture.mtls_target(builder, &url)?,
        None => {
            #[cfg(test)]
            let (builder, url) = super::test_support::mtls_target(builder, &url);
            (builder, url)
        }
    };
    let http = builder.build().map_err(|_| failure(FAILED))?;
    let mut response = http
        .post(url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
        ])
        .send()
        .await
        .map_err(|_| failure(FAILED))?;
    if !response.status().is_success() {
        let delay = crate::error::retry_after_from_headers(
            response.headers(),
            std::time::SystemTime::now(),
        );
        return Err(super::errors::rejected(
            response.status().as_u16(),
            delay,
            true,
        ));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_TOKEN_BYTES as u64)
    {
        return Err(failure(FAILED));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| failure(FAILED))? {
        if chunk.len() > MAX_TOKEN_BYTES - bytes.len() {
            return Err(failure(FAILED));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
