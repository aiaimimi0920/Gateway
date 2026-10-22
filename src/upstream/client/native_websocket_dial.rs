//! CLI-compatible handshake over a system-proxy-aware, cancellable connection.
use crate::error::{
    classify_native_transport_failure, classify_upstream_error, retry_after_from_headers,
    GatewayError,
};
use axum::http;
use hyper_util::client::{legacy::connect::HttpConnector, proxy::matcher::Matcher};
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_tungstenite_native::{
    client_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream,
};
use tower_service::Service;
use tungstenite_native::{
    proxy::{ProxyAuth, ProxyConfig, ProxyScheme},
    Error,
};

pub(super) trait SocketIo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> SocketIo for T {}
type Io = Box<dyn SocketIo>;
pub(super) type Socket = WebSocketStream<MaybeTlsStream<Io>>;

pub(super) async fn connect(
    builder: reqwest::RequestBuilder,
    provider: &str,
) -> Result<Socket, GatewayError> {
    let mut request = builder
        .build()
        .map_err(|_| GatewayError::bad_request("Invalid WS handshake"))?;
    let timeout = request
        .timeout()
        .copied()
        .unwrap_or(Duration::from_secs(120));
    super::handshake::prepare(
        &mut request,
        &tungstenite_native::handshake::client::generate_key(),
    )?;
    let operation = async {
        let uri = request
            .url()
            .as_str()
            .parse::<http::Uri>()
            .map_err(|_| invalid())?;
        let io = dial(&uri).await?;
        let mut url = request.url().clone();
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme).map_err(|_| invalid())?;
        let mut handshake = http::Request::builder()
            .uri(url.as_str())
            .body(())
            .map_err(|_| invalid())?;
        *handshake.headers_mut() = request.headers().clone();
        let tls = crate::http_client::native_codex_websocket_tls().map_err(|_| invalid())?;
        client_async_tls_with_config(
            handshake,
            io,
            Some(super::deflate::config()),
            Some(Connector::Rustls(Arc::new(tls))),
        )
        .await
        .map(|(socket, _)| socket)
    };
    match tokio::time::timeout(timeout, operation).await {
        Ok(Ok(socket)) => Ok(socket),
        Err(_) => Err(classify_native_transport_failure(true, provider)),
        Ok(Err(Error::Http(response))) => {
            let delay = retry_after_from_headers(response.headers(), SystemTime::now());
            let body = response.body().as_deref().unwrap_or_default();
            Err(classify_upstream_error(
                response.status().as_u16(),
                &String::from_utf8_lossy(body),
                Some(provider),
            )
            .with_retry_after_ms(delay))
        }
        Ok(Err(Error::Protocol(_) | Error::Utf8(_))) => Err(GatewayError::bad_request(
            "Invalid upstream WebSocket handshake",
        )
        .with_provider(provider)),
        Ok(Err(_)) => Err(classify_native_transport_failure(false, provider)),
    }
}

fn invalid() -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "Invalid WebSocket transport configuration",
    ))
}

async fn dial(target: &http::Uri) -> Result<Io, Error> {
    // Same environment/system/NO_PROXY matcher as reqwest; never forward proxy
    // authorization to the origin, or origin authorization to the CONNECT hop.
    let proxy = Matcher::from_system().intercept(target);
    let endpoint = proxy.as_ref().map_or(target, |p| p.uri());
    let host = endpoint
        .host()
        .ok_or_else(invalid)?
        .trim_matches(['[', ']']);
    let port = endpoint.port_u16().unwrap_or(match endpoint.scheme_str() {
        Some("https") => 443,
        Some("socks5" | "socks5h") => 1080,
        _ => 80,
    });
    let authority = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let uri = format!("http://{authority}/")
        .parse::<http::Uri>()
        .map_err(|_| invalid())?;
    let mut connector = HttpConnector::new();
    connector.set_nodelay(false);
    connector.set_happy_eyeballs_timeout(Some(Duration::from_millis(250)));
    let mut io: Io = Box::new(
        connector
            .call(uri)
            .await
            .map_err(|_| invalid())?
            .into_inner(),
    );
    let Some(proxy) = proxy.as_ref() else {
        return Ok(io);
    };
    if endpoint.scheme_str() == Some("https") {
        let tls = crate::http_client::native_codex_websocket_tls().map_err(|_| invalid())?;
        let name =
            rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(|_| invalid())?;
        io = Box::new(
            tokio_rustls::TlsConnector::from(Arc::new(tls))
                .connect(name, io)
                .await?,
        );
    }
    let target_host = target.host().ok_or_else(invalid)?;
    let target_port = target
        .port_u16()
        .unwrap_or(if target.scheme_str() == Some("https") {
            443
        } else {
            80
        });
    match endpoint.scheme_str() {
        Some("http" | "https") => {
            tunnel(&mut io, target_host, target_port, proxy.basic_auth()).await?
        }
        Some("socks5" | "socks5h") => {
            let config = ProxyConfig {
                scheme: if endpoint.scheme_str() == Some("socks5") {
                    ProxyScheme::Socks5
                } else {
                    ProxyScheme::Socks5h
                },
                host: host.into(),
                port,
                auth: proxy.raw_auth().map(|(u, p)| ProxyAuth {
                    username: u.into(),
                    password: p.into(),
                }),
            };
            let host = target_host.trim_matches(['[', ']']);
            // socks5 resolves locally; socks5h delegates DNS to the proxy.
            let resolved = if endpoint.scheme_str() == Some("socks5") {
                Some(
                    tokio::net::lookup_host((host, target_port))
                        .await?
                        .next()
                        .ok_or_else(invalid)?
                        .ip()
                        .to_string(),
                )
            } else {
                None
            };
            io = tokio_tungstenite_native::proxy::connect_via_proxy(
                io,
                &config,
                resolved.as_deref().unwrap_or(host),
                target_port,
            )
            .await?;
        }
        _ => return Err(invalid()),
    }
    Ok(io)
}

async fn tunnel(
    io: &mut Io,
    host: &str,
    port: u16,
    auth: Option<&http::HeaderValue>,
) -> Result<(), Error> {
    let authority = format!("{host}:{port}");
    let mut request = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n").into_bytes();
    if let Some(auth) = auth {
        request.extend_from_slice(b"Proxy-Authorization: ");
        request.extend_from_slice(auth.as_bytes());
        request.extend_from_slice(b"\r\n");
    }
    request.extend_from_slice(b"\r\n");
    io.write_all(&request).await?;
    io.flush().await?;
    // Read only through the terminator: never discard tunneled TLS/WS bytes.
    let mut response = Vec::with_capacity(512);
    while response.len() < 8192 {
        response.push(io.read_u8().await?);
        if response.ends_with(b"\r\n\r\n") {
            let status = tungstenite_native::proxy::parse_http_connect_response(&response)?;
            return if (200..300).contains(&status) {
                Ok(())
            } else {
                Err(invalid())
            };
        }
    }
    Err(invalid())
}

#[cfg(test)]
#[path = "native_websocket_proxy_tests.rs"]
mod tests;
