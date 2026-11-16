//! Dedicated no-cookie client with retries disabled, fixed destination and one decoded-byte deadline.
use super::{LoginError, Reply};
use std::time::Duration;

pub struct LoginTransport(rquest::Client);
impl LoginTransport {
    pub fn new() -> Result<Self, LoginError> {
        let builder = crate::http_client::builder().retry(rquest::retry::Policy::never());
        #[cfg(test)]
        let builder = builder.no_proxy();
        builder.build().map(Self).map_err(|_| LoginError::Transport)
    }
}
impl std::fmt::Debug for LoginTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffLoginTransport")
    }
}

pub(super) enum Request<'a> {
    Code {
        fingerprint: &'a str,
    },
    Status {
        fingerprint: &'a str,
        hash: &'a str,
        expiry: &'a str,
    },
}

#[cfg(feature = "line-freebuff-web-reverse-api")]
pub(super) async fn exchange(
    http: &LoginTransport,
    request: Request<'_>,
    budget: Duration,
) -> Result<Reply, LoginError> {
    use futures::StreamExt;
    let origin = "https://freebuff.com".to_owned();
    #[cfg(test)]
    let origin = super::TEST_ORIGIN
        .try_with(Clone::clone)
        .map(|origin| {
            let url = url::Url::parse(&origin).unwrap();
            assert_eq!(url.scheme(), "http");
            assert_eq!(url.host_str(), Some("127.0.0.1"));
            assert!(url.port().is_some() && url.path() == "/");
            assert!(url.query().is_none() && url.fragment().is_none());
            origin
        })
        .unwrap_or(origin);
    tokio::time::timeout(budget, async {
        let request = match request {
            Request::Code { fingerprint } => http
                .0
                .post(format!("{origin}/api/auth/cli/code"))
                .json(&serde_json::json!({"fingerprintId":fingerprint})),
            Request::Status {
                fingerprint,
                hash,
                expiry,
            } => http.0.get(format!("{origin}/api/auth/cli/status")).query(&[
                ("fingerprintId", fingerprint),
                ("fingerprintHash", hash),
                ("expiresAt", expiry),
            ]),
        };
        let response = request
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header("accept", "application/json")
            .send()
            .await
            .map_err(classify)?;
        if response.status().is_redirection() {
            return Err(LoginError::RedirectRejected);
        }
        if response
            .content_length()
            .is_some_and(|length| length > 65_536)
        {
            return Err(LoginError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        #[cfg(test)]
        let _ = super::TEST_BODY_STARTED.try_with(|started| started.notify_one());
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(classify)?;
            if bytes.len().saturating_add(chunk.len()) > 65_536 {
                return Err(LoginError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| LoginError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        Ok(Reply { status, bytes })
    })
    .await
    .map_err(|_| LoginError::Timeout)?
}

#[cfg(feature = "line-freebuff-web-reverse-api")]
fn classify(error: rquest::Error) -> LoginError {
    if error.is_timeout() {
        LoginError::Timeout
    } else {
        LoginError::Transport
    }
}

#[cfg(not(feature = "line-freebuff-web-reverse-api"))]
pub(super) async fn exchange(
    http: &LoginTransport,
    request: Request<'_>,
    budget: Duration,
) -> Result<Reply, LoginError> {
    let _ = (http, request, budget);
    Err(LoginError::FeatureDisabled)
}
