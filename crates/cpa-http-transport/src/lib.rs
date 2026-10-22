//! Opt-in CPA HTTP transport. No credentials, redirects or automatic request retries.
mod body_limit;
mod dial;
mod proxy;
#[cfg(test)]
mod proxy_tests;
pub use body_limit::{is_body_limit_error, BodyLimitExceeded};
mod initialization;
pub use initialization::Initialization;
mod pool;
mod response;
mod wire;
#[cfg(test)]
mod wire_edge_tests;

use http_body::Body as _;
use hyper_util::client::proxy::matcher::Matcher;
use rama_boring::ssl::SslConnector;
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{sync::Semaphore, time::Instant};

/// Deliberately excludes URLs, proxy passwords, headers and native error strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Trust,
    Target,
    UnsupportedProxy,
    Network,
    Timeout,
    Capacity,
    Body,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Trust => "CPA trust configuration failed",
            Self::Target => "CPA Rust transport target is not supported",
            Self::UnsupportedProxy => "CPA Rust transport proxy is not supported",
            Self::Network => "CPA Rust transport failed",
            Self::Timeout => "CPA Rust transport deadline exceeded",
            Self::Capacity => "CPA Rust transport capacity exceeded",
            Self::Body => "CPA Rust transport requires a bounded buffered request body",
        })
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// Share one instance to preserve connection reuse. Plain HTTP is loopback-test-only.
pub struct Client {
    tls: SslConnector,
    proxies: Matcher,
    pool: pool::Pool,
    admission: Arc<Semaphore>,
    allow_http_loopback: bool,
}
impl Client {
    /// Load system trust plus an optional explicit test/private CA. Never disables verification.
    pub fn new(extra_ca: Option<&Path>, allow_http_loopback: bool) -> Result<Self> {
        Ok(Self {
            tls: dial::connector(extra_ca)?,
            proxies: Matcher::from_system(),
            pool: pool::Pool::default(),
            admission: pool::admission(),
            allow_http_loopback,
        })
    }

    /// No redirects or retries: replaying a paid model request is caller-owned.
    pub async fn send(
        &self,
        request: rquest::Request,
        timeout: Duration,
    ) -> Result<rquest::Response> {
        let uri = request.uri().clone();
        let method = request.method().clone();
        let host = uri.host().ok_or(Error::Target)?;
        let plain_test = uri.scheme_str() == Some("http")
            && self.allow_http_loopback
            && matches!(host, "127.0.0.1" | "[::1]");
        if (uri.scheme_str() != Some("https") && !plain_test)
            || uri.authority().is_none_or(|a| a.as_str().contains('@'))
            || matches!(
                host,
                "api.anthropic.com" | "chatgpt.com" | "chat.openai.com"
            )
        {
            return Err(Error::Target);
        }
        if let Some(body) = request.body() {
            if body
                .size_hint()
                .exact()
                .is_none_or(|size| size > 32 * 1024 * 1024)
            {
                return Err(Error::Body);
            }
        }
        let deadline = Instant::now() + timeout;
        let admission = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Capacity)?;
        let (response, lease) = tokio::time::timeout_at(
            deadline,
            self.pool
                .send(&uri, request.into(), &self.tls, &self.proxies, admission),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        Ok(response::adapt(response, uri, method, lease, deadline))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod response_tests;

#[cfg(test)]
mod lifecycle_tests;
