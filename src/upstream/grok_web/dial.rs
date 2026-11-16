//! Upgrade the existing wreq connection: same Chrome TLS, proxy and account material as identity GET.
use super::{failure, machine::FRAME_LIMIT, session};
use crate::error::{ErrorKind, GatewayError};
use futures::StreamExt;
use rquest::header::{HeaderMap, HeaderValue};
use std::time::Duration;
use tokio_tungstenite::{
    tungstenite::{
        handshake::derive_accept_key,
        protocol::{Role, WebSocketConfig},
    },
    WebSocketStream,
};
pub(super) type Socket = WebSocketStream<rquest::Upgraded>;

pub(super) async fn open(
    client: &rquest::Client,
    mut headers: HeaderMap,
) -> Result<Socket, GatewayError> {
    let identity = endpoint("/api/auth/session");
    let user = tokio::time::timeout(Duration::from_secs(15), async {
        let response = client
            .get(identity)
            .headers(headers.clone())
            .redirect(rquest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(network)?;
        admit(response.status().as_u16(), false)?;
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(network)?;
            if chunk.len() > 64 * 1024 - bytes.len() {
                return Err(failure("grok_web_identity_limit", ErrorKind::ServerError));
            }
            bytes.extend_from_slice(&chunk);
        }
        session::user_id(&bytes)
    })
    .await
    .map_err(|_| timeout())??;
    headers.insert(
        "x-userid",
        HeaderValue::from_str(&user).map_err(|_| super::machine::protocol())?,
    );
    let nonce = tokio_tungstenite::tungstenite::handshake::client::generate_key();
    for (key, value) in [
        ("connection", "Upgrade"),
        ("upgrade", "websocket"),
        ("sec-websocket-version", "13"),
        ("sec-websocket-key", nonce.as_str()),
    ] {
        headers.insert(
            key,
            HeaderValue::from_str(value).map_err(|_| super::machine::protocol())?,
        );
    }
    tokio::time::timeout(Duration::from_secs(20), async {
        let response = client
            .get(format!("{}?uid={user}", endpoint("/ws/mgw/")))
            .version(rquest::Version::HTTP_11)
            .headers(headers)
            .redirect(rquest::redirect::Policy::none())
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(network)?;
        admit(response.status().as_u16(), true)?;
        validate_handshake(response.headers(), &nonce)?;
        let io = response.upgrade().await.map_err(network)?;
        Ok(WebSocketStream::from_raw_socket(
            io,
            Role::Client,
            Some(WebSocketConfig {
                max_message_size: Some(FRAME_LIMIT),
                max_frame_size: Some(FRAME_LIMIT),
                max_write_buffer_size: 2 * FRAME_LIMIT,
                ..Default::default()
            }),
        )
        .await)
    })
    .await
    .map_err(|_| timeout())?
}

pub(super) fn validate_handshake(headers: &HeaderMap, nonce: &str) -> Result<(), GatewayError> {
    let token = |key, expected: &str| {
        headers
            .get_all(key)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|s| s.split(','))
            .any(|s| s.trim().eq_ignore_ascii_case(expected))
    };
    if !token("connection", "upgrade")
        || !token("upgrade", "websocket")
        || headers.get_all("sec-websocket-accept").iter().count() != 1
        || headers
            .get("sec-websocket-accept")
            .and_then(|v| v.to_str().ok())
            != Some(derive_accept_key(nonce.as_bytes()).as_str())
        || headers.contains_key("sec-websocket-extensions")
        || headers.contains_key("sec-websocket-protocol")
    {
        return Err(failure(
            "grok_web_handshake_invalid",
            ErrorKind::ServerError,
        ));
    }
    Ok(())
}
fn endpoint(path: &str) -> String {
    #[cfg(test)]
    {
        return super::test_support::target(path);
    }
    #[cfg(not(test))]
    {
        format!("https://grok.com{path}")
    }
}
fn admit(status: u16, upgrade: bool) -> Result<(), GatewayError> {
    if status == if upgrade { 101 } else { 200 } {
        return Ok(());
    }
    let (code, kind) = match status {
        401 | 403 => ("grok_web_authentication", ErrorKind::Authentication),
        429 => ("grok_web_rate_limit", ErrorKind::RateLimit),
        300..=399 => ("grok_web_redirect", ErrorKind::ServerError),
        _ => ("grok_web_http_failed", ErrorKind::ServerError),
    };
    let mut error = failure(code, kind);
    error.http_status = Some(status);
    Err(error)
}
pub(super) fn timeout() -> GatewayError {
    let mut error = failure("grok_web_timeout", ErrorKind::Timeout);
    error.http_status = Some(504);
    error
}
fn network(error: rquest::Error) -> GatewayError {
    if error.is_timeout() {
        timeout()
    } else {
        failure("grok_web_network", ErrorKind::Network)
    }
}
