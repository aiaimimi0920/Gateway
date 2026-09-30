//! One loopback callback listener, released on completion, cancellation, or timeout.
use super::Session;
use axum::{extract::Query, response::Html, routing::get, Router};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
}

pub(super) fn serve(
    listener: tokio::net::TcpListener,
    session: Arc<Session>,
    client: rquest::Client,
) {
    let stop = session.cancel.clone();
    let app = Router::new().route(
        "/auth/callback",
        get(move |Query(query): Query<Callback>| {
            let session = session.clone();
            let client = client.clone();
            async move {
                let result = session
                    .complete(
                        &client,
                        query.code.as_deref().unwrap_or_default(),
                        query.state.as_deref().unwrap_or_default(),
                    )
                    .await;
                let body = match result {
                    Ok(()) => "ChatGPT authorization received. Return to Gateway to finish saving.",
                    Err(error) if error.code.as_deref() == Some("chatgpt_oauth_transport_failed") =>
                        "Gateway could not reach ChatGPT to exchange the authorization code. Return to Gateway and retry login after checking the network.",
                    Err(_) => "Authorization could not be completed. Return to Gateway for the error details.",
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
                    Html(body),
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

pub fn parse_callback(value: &str) -> Result<(String, String), crate::error::GatewayError> {
    let invalid = || {
        crate::error::GatewayError::bad_request(
            "Paste the complete localhost:1455/auth/callback URL from this login",
        )
    };
    if value.len() > 16_384 {
        return Err(invalid());
    }
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "http"
        || url.host_str() != Some("localhost")
        || url.port() != Some(1455)
        || url.path() != "/auth/callback"
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let field = |key| {
        let values: Vec<_> = pairs.iter().filter(|(k, _)| k == key).collect();
        if values.len() != 1 {
            return Err(invalid());
        }
        Ok(values[0].1.to_string())
    };
    Ok((field("code")?, field("state")?))
}
