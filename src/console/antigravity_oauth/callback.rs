//! One loopback listener, released on completion/cancel/expiry. Manual callbacks share parsing.
use super::Session;
use crate::error::GatewayError;
use axum::{extract::RawQuery, response::Html, routing::get, Router};
use std::sync::Arc;

fn fields(query: &str) -> Result<(String, String), GatewayError> {
    let invalid = || {
        GatewayError::bad_request("OAuth callback requires one code and one state from this login")
    };
    if query.len() > 16384 {
        return Err(invalid());
    }
    let pairs: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
    let field = |key| {
        let values: Vec<_> = pairs.iter().filter(|(k, _)| k == key).collect();
        if values.len() != 1 || values[0].1.is_empty() {
            return Err(invalid());
        }
        Ok(values[0].1.to_string())
    };
    Ok((field("code")?, field("state")?))
}
pub fn parse_callback(value: &str) -> Result<(String, String), GatewayError> {
    let invalid = || {
        GatewayError::bad_request(
            "Paste the complete localhost:51121/oauth-callback URL from this login",
        )
    };
    if value.len() > 16384 {
        return Err(invalid());
    }
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "http"
        || url.host_str() != Some("localhost")
        || url.port() != Some(51121)
        || url.path() != "/oauth-callback"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    fields(url.query().unwrap_or_default())
}
pub(super) fn serve(
    listener: tokio::net::TcpListener,
    session: Arc<Session>,
    client: rquest::Client,
) {
    let stop = session.cancel.clone();
    let app = Router::new().route(
        "/oauth-callback",
        get(move |RawQuery(query): RawQuery| {
            let session = session.clone();
            let client = client.clone();
            async move {
                let result = match fields(query.as_deref().unwrap_or_default()) {
                    Ok((code, nonce)) => session.complete(&client, &code, &nonce).await,
                    Err(e) => Err(e),
                };
                let message = if result.is_ok() {
                    "Antigravity authorization received. Return to Gateway and save the account."
                } else {
                    "Authorization could not be completed. Return to Gateway for status."
                };
                (
                    [
                        ("Cache-Control", "no-store"),
                        (
                            "Content-Security-Policy",
                            "default-src 'none'; frame-ancestors 'none'",
                        ),
                        ("Referrer-Policy", "no-referrer"),
                    ],
                    Html(message),
                )
            }
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                stop.notified().await;
            })
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::antigravity as config;
    #[test]
    fn antigravity_callback_rejects_origin_and_duplicate_field_confusion() {
        assert_eq!(
            parse_callback(&format!("{}?code=c&state=s", config::REDIRECT_URI)).unwrap(),
            ("c".into(), "s".into())
        );
        for value in [
            "http://localhost:51121/oauth-callback?code=c&state=s&state=x",
            "http://localhost:51121/oauth-callback?code=c&code=x&state=s",
            "http://localhost:51121/oauth-callback?code=c&state=s#fragment",
            "http://localhost:1455/auth/callback?code=c&state=s",
            "https://evil.invalid/oauth-callback?code=c&state=s",
        ] {
            assert!(parse_callback(value).is_err());
        }
    }
}
