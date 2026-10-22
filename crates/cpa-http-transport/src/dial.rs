//! Verified TLS and bounded CONNECT. Proxy credentials never enter origin headers.
use crate::{Error, Result};
use http::Uri;
use hyper_util::client::proxy::matcher::Matcher;
use rama_boring::{
    ssl::{SslConnector, SslMethod},
    x509::X509,
};
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite};

pub(crate) trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Socket for T {}
pub(crate) type Io = Box<dyn Socket>;

pub(crate) fn connector(extra_ca: Option<&Path>) -> Result<SslConnector> {
    let mut builder = SslConnector::builder(SslMethod::tls_client()).map_err(|_| Error::Trust)?;
    let roots = rustls_native_certs::load_native_certs();
    if roots.certs.is_empty() {
        return Err(Error::Trust);
    }
    for cert in roots.certs {
        let cert = X509::from_der(cert.as_ref()).map_err(|_| Error::Trust)?;
        builder
            .cert_store_mut()
            .add_cert(cert)
            .map_err(|_| Error::Trust)?;
    }
    if let Some(path) = extra_ca {
        let metadata = std::fs::metadata(path).map_err(|_| Error::Trust)?;
        if metadata.len() > 1024 * 1024 {
            return Err(Error::Trust);
        }
        let bytes = std::fs::read(path).map_err(|_| Error::Trust)?;
        let certs = X509::stack_from_pem(&bytes).map_err(|_| Error::Trust)?;
        if certs.is_empty() {
            return Err(Error::Trust);
        }
        for cert in certs {
            builder
                .cert_store_mut()
                .add_cert(cert)
                .map_err(|_| Error::Trust)?;
        }
    }
    gateway_cpa_tls_profile::configure_context(&mut builder).map_err(|_| Error::Trust)?;
    Ok(builder.build())
}

pub(crate) async fn connect(
    target: &Uri,
    tls: &SslConnector,
    proxies: &Matcher,
) -> Result<(Io, bool)> {
    let proxy = proxies.intercept(target);
    let endpoint = proxy.as_ref().map_or(target, |p| p.uri());
    let scheme = endpoint.scheme_str().ok_or(Error::Target)?;
    let default_port = match scheme {
        "http" => 80,
        "https" => 443,
        "socks5" | "socks5h" if proxy.is_some() => 1080,
        _ => return Err(Error::UnsupportedProxy),
    };
    // HTTP forward proxies need absolute-form, which this origin transport does not emit.
    if target.scheme_str() == Some("http") && proxy.is_some() && matches!(scheme, "http" | "https")
    {
        return Err(Error::UnsupportedProxy);
    }
    let host = endpoint
        .host()
        .ok_or(Error::Target)?
        .trim_matches(['[', ']']);
    let tcp = tokio::net::TcpStream::connect((host, endpoint.port_u16().unwrap_or(default_port)))
        .await
        .map_err(|_| Error::Network)?;
    tcp.set_nodelay(true).map_err(|_| Error::Network)?;
    let mut io: Io = Box::new(tcp);
    if let Some(proxy) = proxy.as_ref() {
        if scheme == "https" {
            // Verify the proxy independently before the inner origin TLS handshake.
            io = tls_stream(io, tls, host).await?.0;
        }
        match scheme {
            "http" | "https" => crate::proxy::connect(&mut io, target, proxy.basic_auth()).await?,
            "socks5" | "socks5h" => crate::proxy::socks5(&mut io, target, proxy.raw_auth()).await?,
            _ => return Err(Error::UnsupportedProxy),
        }
    }
    if target.scheme_str() == Some("http") {
        return Ok((io, false));
    }
    tls_stream(io, tls, target.host().ok_or(Error::Target)?).await
}

async fn tls_stream(io: Io, tls: &SslConnector, host: &str) -> Result<(Io, bool)> {
    let mut ssl = tls
        .configure()
        .and_then(|c| c.into_ssl(Some(host.trim_matches(['[', ']']))))
        .map_err(|_| Error::Trust)?;
    gateway_cpa_tls_profile::configure_connection(&mut ssl).map_err(|_| Error::Trust)?;
    let stream = rama_boring_tokio::SslStreamBuilder::new(ssl, io)
        .connect()
        .await
        .map_err(|_| Error::Network)?;
    let h2 = stream.ssl().selected_alpn_protocol() == Some(b"h2");
    Ok((Box::new(stream), h2))
}
