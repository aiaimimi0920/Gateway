//! Bounded OAuth exchange. Neither upstream bodies nor token material enter errors/logs.
use futures::StreamExt;
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Deserialize)]
pub(super) struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
}

pub(super) struct RefreshError {
    pub code: &'static str,
    pub terminal: bool,
}

impl RefreshError {
    fn transient(code: &'static str) -> Self {
        Self {
            code,
            terminal: false,
        }
    }
}

pub(super) async fn exchange(
    http: &rquest::Client,
    endpoint: &str,
    refresh_token: &str,
    client_id: &str,
    antigravity: bool,
) -> Result<TokenResponse, RefreshError> {
    // Identify by adapter, never by an ID coincidence. Config failures are recoverable
    // and must not send a refresh token or mark an existing account permanently invalid.
    let oauth_client = if antigravity {
        if endpoint != crate::protocol::antigravity::TOKEN_ENDPOINT {
            return Err(RefreshError::transient("antigravity_oauth_client_mismatch"));
        }
        Some(
            crate::protocol::antigravity::OAuthClient::load()
                .await
                .map_err(RefreshError::transient)?,
        )
    } else {
        None
    };
    let antigravity_secret = oauth_client
        .as_ref()
        .map(|client| client.refresh_secret(endpoint, client_id))
        .transpose()
        .map_err(RefreshError::transient)?;
    // An OAuth redirect must never forward a refresh token to another endpoint.
    let url = url::Url::parse(endpoint)
        .map_err(|_| RefreshError::transient("oauth_refresh_invalid_endpoint"))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !(url.scheme() == "https"
            || (url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
    {
        return Err(RefreshError {
            code: "oauth_refresh_unsafe_endpoint",
            terminal: true,
        });
    }
    let mut form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    if let Some(secret) = antigravity_secret {
        form.push(("client_secret", secret));
    }
    let response = http
        .post(url.as_str())
        .default_headers(antigravity_secret.is_none())
        .redirect(rquest::redirect::Policy::none())
        .form(&form)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| RefreshError::transient("oauth_refresh_transport_failed"))?;
    let status = response.status();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| RefreshError::transient("oauth_refresh_body_failed"))?;
        if bytes.len().saturating_add(chunk.len()) > 65_536 {
            return Err(RefreshError::transient("oauth_refresh_response_too_large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let error = serde_json::from_slice::<serde_json::Value>(&bytes).ok();
        let kind = error
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(|v| v.as_str());
        let terminal = matches!(
            kind,
            Some("invalid_grant" | "invalid_client" | "unauthorized_client")
        );
        return Err(RefreshError {
            code: if terminal {
                "oauth_reauthorization_required"
            } else {
                "oauth_refresh_rejected"
            },
            terminal,
        });
    }
    let mut token: TokenResponse = serde_json::from_slice(&bytes)
        .map_err(|_| RefreshError::transient("oauth_refresh_invalid_response"))?;
    if token.access_token.trim().is_empty() || token.expires_in == Some(0) {
        return Err(RefreshError::transient("oauth_refresh_invalid_response"));
    }
    if token
        .refresh_token
        .as_deref()
        .is_some_and(|s| s.trim().is_empty())
    {
        token.refresh_token = None;
    }
    Ok(token)
}
