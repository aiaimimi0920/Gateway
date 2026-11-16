//! One fresh fixed-origin JWKS read per identity verification; no background cache or retry loop.
use super::super::{OAuthClient, OAuthError};
use futures::StreamExt;
use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(10);
const MAX_BYTES: usize = 65_536;
pub(super) async fn key_set(
    client: &OAuthClient,
    http: &rquest::Client,
) -> Result<Vec<u8>, OAuthError> {
    tokio::time::timeout(DEADLINE, async {
        let reply = http
            .get(format!("{}/.well-known/jwks.json", client.issuer))
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(DEADLINE)
            .send()
            .await
            .map_err(|_| OAuthError::Transport)?;
        if reply.status().is_redirection() {
            return Err(OAuthError::RedirectRejected);
        }
        if !reply.status().is_success() {
            return Err(OAuthError::Rejected);
        }
        if reply.content_length().is_some_and(|n| n > MAX_BYTES as u64) {
            return Err(OAuthError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut stream = reply.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| OAuthError::Transport)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
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
