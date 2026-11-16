//! One fixed-directory JSON request; inherited credentials, redirects and automatic retries disabled.
use super::{Directory, OAuthError, Reply};
use futures::StreamExt;
use serde_json::Value;
use std::time::Duration;

pub(super) async fn post(
    http: &rquest::Client,
    directory: &Directory,
    path: &'static str,
    body: Value,
    budget: Duration,
) -> Result<Reply, OAuthError> {
    let origin = directory.oidc_origin();
    #[cfg(test)]
    let origin = super::TEST_ORIGIN
        .try_with(Clone::clone)
        .map(|origin| {
            let url = url::Url::parse(&origin).unwrap();
            assert_eq!(url.scheme(), "http");
            assert_eq!(url.host_str(), Some("127.0.0.1"));
            assert!(url.port().is_some());
            assert_eq!(url.path(), "/");
            assert!(url.query().is_none() && url.fragment().is_none());
            origin
        })
        .unwrap_or(origin);
    tokio::time::timeout(budget, async {
        let response = http
            .post(format!("{origin}{path}"))
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header("accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(classify_transport)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(OAuthError::RedirectRejected);
        }
        if response
            .content_length()
            .is_some_and(|length| length > 65_536)
        {
            return Err(OAuthError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        #[cfg(test)]
        let _ = super::TEST_BODY_STARTED.try_with(|started| started.notify_one());
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(classify_transport)?;
            if bytes.len().saturating_add(chunk.len()) > 65_536 {
                return Err(OAuthError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| OAuthError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        Ok(Reply {
            status: status.as_u16(),
            bytes,
        })
    })
    .await
    .map_err(|_| OAuthError::Timeout)?
}

fn classify_transport(error: rquest::Error) -> OAuthError {
    // The request timer may win the same deadline as the encompassing send+read timer.
    if error.is_timeout() {
        OAuthError::Timeout
    } else {
        OAuthError::Transport
    }
}
