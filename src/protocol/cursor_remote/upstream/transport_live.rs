//! Pure-Rust HTTP CONNECT proxy tunnel, Rustls ALPN `h2` verifier, and full-duplex H2 client.
use super::{
    transport::{
        validate_target, DuplexRequest, DuplexStream, Transport, UnaryRequest, UnaryResponse,
    },
    ProbeError, RunError,
};
use crate::protocol::cursor_remote::runtime::worker::WorkerHooks;
use axum::http::{HeaderName, HeaderValue, Method, Request};
use bytes::Bytes;
use futures::future::BoxFuture;
use gateway_cpa_http_transport::{
    async_compression::tokio::bufread::GzipDecoder,
    http2::{self, RecvStream, SendStream},
};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task::JoinHandle,
    time::{timeout_at, Instant},
};

const USER_AGENT: &str = "Neuro-Gateway-Cursor-Experimental/0.1";
const MAX_PROXY_HEADERS: usize = 4096;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_DUPLEX_FRAMES: usize = 128;
const MAX_DUPLEX_BYTES: usize = 320 * 1024;

pub(crate) struct LiveTransport {
    #[cfg(test)]
    extra_roots: Vec<rustls::pki_types::CertificateDer<'static>>,
}

impl Default for LiveTransport {
    fn default() -> Self {
        Self {
            #[cfg(test)]
            extra_roots: Vec::new(),
        }
    }
}

#[cfg(test)]
impl LiveTransport {
    pub(crate) fn with_test_root(cert: rustls::pki_types::CertificateDer<'static>) -> Self {
        Self {
            extra_roots: vec![cert],
        }
    }
}

impl Transport for LiveTransport {
    fn unary<'a>(
        &'a self,
        proxy_port: u16,
        req: UnaryRequest<'a>,
    ) -> BoxFuture<'a, Result<UnaryResponse, ProbeError>> {
        Box::pin(async move {
            if proxy_port == 0 {
                return Err(ProbeError::new("explicit_loopback_proxy_required"));
            }
            if req.timeout.is_zero() || req.timeout > Duration::from_secs(60) {
                return Err(ProbeError::new("invalid_request_timeout"));
            }
            validate_caller_headers(
                &req.headers
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect::<Vec<_>>(),
            )?;
            let url = validate_target(req.url)?;
            let deadline = Instant::now() + req.timeout;

            timeout_at(deadline, async {
                let (mut sender, _driver) = self.dial_h2(proxy_port, &url).await?;
                let http_req = build_http_request(
                    req.method,
                    &url,
                    &req.headers
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect::<Vec<_>>(),
                    "gzip",
                )?;
                let end_on_headers = req.body.is_empty();
                let (resp_fut, mut send_stream) = sender
                    .send_request(http_req, end_on_headers)
                    .map_err(|_| ProbeError::new("http2_request_failed"))?;
                if !end_on_headers {
                    send_stream
                        .send_data(req.body, true)
                        .map_err(|_| ProbeError::new("http2_request_failed"))?;
                }
                let (parts, mut recv) = resp_fut
                    .await
                    .map_err(|e| map_h2_error(&e, "http2_request_failed"))?
                    .into_parts();
                let status = parts.status.as_u16();
                let content_type = header_token(&parts.headers, "content-type", "");
                let encoding = header_token(&parts.headers, "content-encoding", "identity");
                if !matches!(encoding.as_str(), "identity" | "gzip") {
                    return Err(ProbeError::new("unsupported_http_content_encoding"));
                }
                let mut raw = Vec::new();
                while let Some(chunk) = recv.data().await {
                    let chunk = chunk.map_err(|e| map_h2_error(&e, "http2_stream_closed"))?;
                    recv.flow_control()
                        .release_capacity(chunk.len())
                        .map_err(|_| ProbeError::new("http2_request_failed"))?;
                    if raw.len() + chunk.len() > MAX_RESPONSE_BYTES {
                        return Err(ProbeError::new("response_too_large"));
                    }
                    raw.extend_from_slice(&chunk);
                }
                verify_clean_eof(&mut recv).await?;
                let body = if encoding == "gzip" {
                    decompress_gzip(&raw).await?
                } else {
                    raw
                };
                Ok(UnaryResponse {
                    status,
                    content_type,
                    body,
                })
            })
            .await
            .map_err(|_| ProbeError::new("request_timeout"))?
        })
    }

    fn open_duplex<'a>(
        &'a self,
        proxy_port: u16,
        req: DuplexRequest<'a>,
        hooks: &'a mut WorkerHooks,
    ) -> BoxFuture<'a, Result<Box<dyn DuplexStream>, RunError>> {
        Box::pin(async move {
            if proxy_port == 0 {
                return Err(ProbeError::new("explicit_loopback_proxy_required").into());
            }
            if req.timeout.is_zero() || req.timeout > Duration::from_secs(300) {
                return Err(ProbeError::new("invalid_request_timeout").into());
            }
            validate_caller_headers(req.headers)?;
            let url = validate_target(req.url)?;
            let deadline = Instant::now() + req.timeout;

            let (mut sender, driver) = timeout_at(deadline, self.dial_h2(proxy_port, &url))
                .await
                .map_err(|_| RunError::Probe(ProbeError::new("request_timeout")))??;
            let http_req = build_http_request("POST", &url, req.headers, "identity")?;

            // Re-verify authorization & budget AFTER TLS + H2 connect, BEFORE sending HEADERS.
            timeout_at(deadline, hooks.authorize())
                .await
                .map_err(|_| RunError::Probe(ProbeError::new("request_timeout")))?
                .map_err(RunError::Gateway)?;

            let (resp_fut, mut send_stream) = sender
                .send_request(http_req, false)
                .map_err(|_| RunError::Probe(ProbeError::new("http2_request_failed")))?;
            send_stream
                .send_data(req.body, false)
                .map_err(|_| RunError::Probe(ProbeError::new("http2_request_failed")))?;
            hooks.sent().map_err(RunError::Gateway)?;

            let (parts, recv) = timeout_at(deadline, resp_fut)
                .await
                .map_err(|_| RunError::Probe(ProbeError::new("request_timeout")))?
                .map_err(|e| RunError::Probe(map_h2_error(&e, "http2_request_failed")))?
                .into_parts();

            let status = parts.status.as_u16();
            let content_type = header_token(&parts.headers, "content-type", "");
            let encoding = header_token(&parts.headers, "content-encoding", "identity");
            if encoding != "identity" {
                return Err(ProbeError::new("unsupported_http_content_encoding").into());
            }
            if status == 200 && content_type != "application/connect+proto" {
                return Err(ProbeError::new("unexpected_stream_content_type").into());
            }

            Ok(Box::new(LiveDuplexStream {
                status,
                deadline,
                send_stream,
                recv,
                _driver: DriverGuard(driver),
                received_bytes: 0,
                sent_frames: 0,
                sent_bytes: 0,
                eof_verified: false,
            }) as Box<dyn DuplexStream>)
        })
    }
}

impl LiveTransport {
    async fn dial_h2(
        &self,
        proxy_port: u16,
        url: &url::Url,
    ) -> Result<(http2::client::SendRequest<Bytes>, JoinHandle<()>), ProbeError> {
        let host = url
            .host_str()
            .ok_or_else(|| ProbeError::new("invalid_target"))?;
        let mut tcp = TcpStream::connect(("127.0.0.1", proxy_port))
            .await
            .map_err(|_| ProbeError::new("proxy_connection_failed"))?;
        let _ = tcp.set_nodelay(true);
        proxy_connect(&mut tcp, host).await?;

        let tls_cfg = self.tls_config()?;
        let server_name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|_| ProbeError::new("invalid_target"))?;
        let tls = tokio_rustls::TlsConnector::from(tls_cfg)
            .connect(server_name, tcp)
            .await
            .map_err(|_| ProbeError::new("verified_http2_required"))?;
        if tls.get_ref().1.alpn_protocol() != Some(b"h2") {
            return Err(ProbeError::new("verified_http2_required"));
        }

        let (sender, conn) = http2::client::Builder::new()
            .enable_push(false)
            .max_send_buffer_size(4 * 1024 * 1024)
            .handshake::<_, Bytes>(tls)
            .await
            .map_err(|_| ProbeError::new("http2_request_failed"))?;
        let driver = tokio::spawn(async move {
            let _ = conn.await;
        });
        let ready = sender
            .ready()
            .await
            .map_err(|_| ProbeError::new("http2_request_failed"))?;
        Ok((ready, driver))
    }

    fn tls_config(&self) -> Result<Arc<rustls::ClientConfig>, ProbeError> {
        #[cfg(test)]
        if !self.extra_roots.is_empty() {
            return build_tls_config(&self.extra_roots);
        }
        static PROD_TLS: OnceLock<Result<Arc<rustls::ClientConfig>, &'static str>> =
            OnceLock::new();
        PROD_TLS
            .get_or_init(|| build_tls_config(&[]).map_err(|e| e.code))
            .clone()
            .map_err(ProbeError::new)
    }
}

fn build_tls_config(
    extra: &[rustls::pki_types::CertificateDer<'static>],
) -> Result<Arc<rustls::ClientConfig>, ProbeError> {
    let mut roots = rustls::RootCertStore::empty();
    if extra.is_empty() {
        let native = rustls_native_certs::load_native_certs();
        for cert in native.certs {
            let _ = roots.add(cert);
        }
    } else {
        for cert in extra {
            roots
                .add(cert.clone())
                .map_err(|_| ProbeError::new("verified_http2_required"))?;
        }
    }
    if roots.is_empty() {
        return Err(ProbeError::new("verified_http2_required"));
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|_| ProbeError::new("verified_http2_required"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"h2".to_vec()];
    Ok(Arc::new(cfg))
}

async fn proxy_connect(tcp: &mut TcpStream, host: &str) -> Result<(), ProbeError> {
    let head = format!("CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n");
    tcp.write_all(head.as_bytes())
        .await
        .map_err(|_| ProbeError::new("proxy_connection_failed"))?;
    let mut buf = Vec::with_capacity(128);
    while !buf.ends_with(b"\r\n\r\n") {
        if buf.len() >= MAX_PROXY_HEADERS {
            return Err(ProbeError::new("proxy_headers_too_large"));
        }
        match tcp.read_u8().await {
            Ok(byte) => buf.push(byte),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(ProbeError::new("proxy_truncated"));
            }
            Err(_) => return Err(ProbeError::new("proxy_connection_failed")),
        }
    }
    let ok = buf.starts_with(b"HTTP/1.1 200 ")
        || buf.starts_with(b"HTTP/1.1 200\r\n")
        || buf.starts_with(b"HTTP/1.0 200 ")
        || buf.starts_with(b"HTTP/1.0 200\r\n");
    if !ok {
        return Err(ProbeError::new("proxy_connect_rejected"));
    }
    Ok(())
}

fn validate_caller_headers(headers: &[(&str, &str)]) -> Result<(), ProbeError> {
    for &(key, _) in headers {
        if key.starts_with(':')
            || key.eq_ignore_ascii_case("host")
            || key.eq_ignore_ascii_case("user-agent")
        {
            return Err(ProbeError::new("reserved_request_header"));
        }
    }
    Ok(())
}

fn build_http_request(
    method: &str,
    url: &url::Url,
    headers: &[(&str, &str)],
    accept_encoding: &str,
) -> Result<Request<()>, ProbeError> {
    let mut builder = Request::builder()
        .method(
            Method::from_bytes(method.as_bytes()).map_err(|_| ProbeError::new("invalid_target"))?,
        )
        .uri(url.as_str())
        .header("user-agent", USER_AGENT)
        .header("accept-encoding", accept_encoding);
    let map = builder
        .headers_mut()
        .ok_or_else(|| ProbeError::new("http2_request_failed"))?;
    for &(k, v) in headers {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| ProbeError::new("http2_request_failed"))?;
        let val = HeaderValue::from_str(v).map_err(|_| ProbeError::new("http2_request_failed"))?;
        map.insert(name, val);
    }
    builder
        .body(())
        .map_err(|_| ProbeError::new("http2_request_failed"))
}

fn header_token(headers: &axum::http::HeaderMap, key: &str, fallback: &str) -> String {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .unwrap_or(fallback)
        .split(';')
        .next()
        .unwrap_or(fallback)
        .trim()
        .to_ascii_lowercase()
}

fn map_h2_error(err: &http2::Error, default_code: &'static str) -> ProbeError {
    if err.is_reset() {
        ProbeError::new("http2_reset")
    } else {
        ProbeError::new(default_code)
    }
}

/// Per RFC 9113 Section 8.1, an HTTP/2 server closing a full-duplex stream after sending its
/// `END_STREAM` response while the client request side remains open sends `RST_STREAM(NO_ERROR)`
/// (`rstCode == 0`). Non-zero reset reasons still fail closed as `http2_reset`, and Connect
/// `EndStream` + `TurnEnded` framing is still verified independently by the decoder.
fn is_duplex_no_error_reset(err: &http2::Error) -> bool {
    err.is_reset() && err.is_remote() && err.reason() == Some(http2::Reason::NO_ERROR)
}

async fn verify_clean_eof(recv: &mut RecvStream) -> Result<(), ProbeError> {
    let _ = recv
        .trailers()
        .await
        .map_err(|e| map_h2_error(&e, "http2_stream_closed"))?;
    if !recv.is_end_stream() {
        return Err(ProbeError::new("http2_stream_closed"));
    }
    Ok(())
}

async fn decompress_gzip(compressed: &[u8]) -> Result<Vec<u8>, ProbeError> {
    let mut decoder = GzipDecoder::new(std::io::Cursor::new(compressed));
    decoder.multiple_members(true);
    let mut out = Vec::new();
    tokio::io::AsyncReadExt::take(&mut decoder, (MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut out)
        .await
        .map_err(|_| ProbeError::new("invalid_compressed_response"))?;
    if out.len() > MAX_RESPONSE_BYTES {
        return Err(ProbeError::new("invalid_compressed_response"));
    }
    Ok(out)
}

struct DriverGuard(JoinHandle<()>);
impl Drop for DriverGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct LiveDuplexStream {
    status: u16,
    deadline: Instant,
    send_stream: SendStream<Bytes>,
    recv: RecvStream,
    _driver: DriverGuard,
    received_bytes: usize,
    sent_frames: usize,
    sent_bytes: usize,
    eof_verified: bool,
}

impl Drop for LiveDuplexStream {
    fn drop(&mut self) {
        self.send_stream.send_reset(http2::Reason::CANCEL);
    }
}

impl DuplexStream for LiveDuplexStream {
    fn status(&self) -> u16 {
        self.status
    }

    fn next_chunk(&mut self) -> BoxFuture<'_, Result<Option<Bytes>, ProbeError>> {
        Box::pin(async move {
            if self.eof_verified {
                return Ok(None);
            }
            timeout_at(self.deadline, async {
                match self.recv.data().await {
                    Some(Ok(chunk)) => {
                        self.recv
                            .flow_control()
                            .release_capacity(chunk.len())
                            .map_err(|_| ProbeError::new("http2_request_failed"))?;
                        self.received_bytes = self.received_bytes.saturating_add(chunk.len());
                        if self.received_bytes > MAX_RESPONSE_BYTES {
                            return Err(ProbeError::new("response_too_large"));
                        }
                        Ok(Some(chunk))
                    }
                    Some(Err(e)) if self.received_bytes > 0 && is_duplex_no_error_reset(&e) => {
                        self.eof_verified = true;
                        Ok(None)
                    }
                    Some(Err(e)) => Err(map_h2_error(&e, "http2_stream_closed")),
                    None => match self.recv.trailers().await {
                        Ok(_) if self.recv.is_end_stream() => {
                            self.eof_verified = true;
                            Ok(None)
                        }
                        Err(e) if self.received_bytes > 0 && is_duplex_no_error_reset(&e) => {
                            self.eof_verified = true;
                            Ok(None)
                        }
                        Ok(_) => Err(ProbeError::new("http2_stream_closed")),
                        Err(e) => Err(map_h2_error(&e, "http2_stream_closed")),
                    },
                }
            })
            .await
            .map_err(|_| ProbeError::new("request_timeout"))?
        })
    }

    fn send_frame(&mut self, frame: Bytes) -> Result<(), ProbeError> {
        if self.eof_verified {
            return Err(ProbeError::new("duplex_write_rejected"));
        }
        self.sent_frames += 1;
        self.sent_bytes = self.sent_bytes.saturating_add(frame.len());
        if self.sent_frames > MAX_DUPLEX_FRAMES || self.sent_bytes > MAX_DUPLEX_BYTES {
            return Err(ProbeError::new("duplex_write_rejected"));
        }
        self.send_stream
            .send_data(frame, false)
            .map_err(|_| ProbeError::new("duplex_write_rejected"))
    }
}
