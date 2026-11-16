//! Shared bounded token-response reader; callers retain their single send/read timeout owner.
use super::OAuthError;
use futures::StreamExt;

pub(super) const LIMIT: usize = 65_536;

pub(super) async fn read_response(response: rquest::Response) -> Result<Vec<u8>, OAuthError> {
    if response.status().is_redirection() {
        return Err(OAuthError::RedirectRejected);
    }
    // Error bodies cannot authorize a retry and are never deserialized or echoed.
    if !response.status().is_success() {
        return Err(OAuthError::Rejected);
    }
    if response
        .content_length()
        .is_some_and(|size| size > LIMIT as u64)
    {
        return Err(OAuthError::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| OAuthError::Transport)?;
        if bytes.len().saturating_add(chunk.len()) > LIMIT {
            return Err(OAuthError::ResponseTooLarge);
        }
        bytes
            .try_reserve(chunk.len())
            .map_err(|_| OAuthError::ResponseTooLarge)?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
