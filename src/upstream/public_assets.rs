//! Untrusted media URLs: credential-free, direct, pinned public DNS and bounded decoded bytes.
#[cfg(feature = "line-suno-web-reverse-api")]
pub(super) mod audio;
pub(super) mod image;
use futures::StreamExt;
use std::net::IpAddr;
#[cfg(not(test))]
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure {
    Target,
    Http(u16),
    Network,
    Timeout,
    Limit,
    Type,
}

pub(super) fn checked_url(raw: &str) -> Result<url::Url, Failure> {
    let url = url::Url::parse(raw).map_err(|_| Failure::Target)?;
    if raw.len() > 16384
        || url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port().is_some()
        || url.host_str().is_none()
        || url
            .host_str()
            .and_then(|s| s.trim_matches(['[', ']']).parse::<IpAddr>().ok())
            .is_some_and(|ip| !crate::http_client::public_addresses::allowed_ip(ip, false))
    {
        return Err(Failure::Target);
    }
    Ok(url)
}

pub(super) async fn download(
    raw: &str,
    deadline: tokio::time::Instant,
    limit: usize,
) -> Result<(Vec<u8>, String), Failure> {
    let url = checked_url(raw)?;
    tokio::time::timeout_at(deadline, async {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(Failure::Timeout);
        }
        #[cfg(not(test))]
        let client = client(&url, remaining).await?;
        #[cfg(test)]
        let client = rquest::Client::builder().no_proxy().build().unwrap();
        #[cfg(test)]
        let url = test_target(&url);
        let response = client
            .get(url.as_str())
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(remaining)
            .send()
            .await
            .map_err(network)?;
        if response.status().as_u16() != 200 {
            return Err(Failure::Http(response.status().as_u16()));
        }
        let mime = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        Ok((read(response, limit).await?, mime))
    })
    .await
    .map_err(|_| Failure::Timeout)?
}

pub(super) async fn read(response: rquest::Response, limit: usize) -> Result<Vec<u8>, Failure> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(Failure::Limit);
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(network)?;
        let next = bytes
            .len()
            .checked_add(chunk.len())
            .filter(|n| *n <= limit)
            .ok_or(Failure::Limit)?;
        bytes
            .try_reserve(next - bytes.len())
            .map_err(|_| Failure::Limit)?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn network(error: rquest::Error) -> Failure {
    if error.is_timeout() {
        Failure::Timeout
    } else {
        Failure::Network
    }
}

#[cfg(not(test))]
async fn client(url: &url::Url, remaining: Duration) -> Result<rquest::Client, Failure> {
    let host = url
        .host_str()
        .ok_or(Failure::Target)?
        .trim_matches(['[', ']']);
    let addresses: Vec<_> = tokio::net::lookup_host((host, 443))
        .await
        .map_err(|_| Failure::Target)?
        .take(17)
        .collect();
    if addresses.is_empty()
        || addresses.len() > 16
        || addresses
            .iter()
            .any(|a| !crate::http_client::public_addresses::allowed_ip(a.ip(), false))
    {
        return Err(Failure::Target);
    }
    // No proxy may resolve the hostname again; TLS still verifies the original hostname.
    rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .resolve_to_addrs(host.to_owned(), addresses)
        .connect_timeout(remaining.min(Duration::from_secs(15)))
        .timeout(remaining)
        .build()
        .map_err(|_| Failure::Target)
}

#[cfg(test)]
tokio::task_local! { static ORIGIN: String; }
#[cfg(test)]
pub(crate) async fn within<T>(origin: &str, future: impl std::future::Future<Output = T>) -> T {
    assert!(origin.starts_with("http://127.0.0.1:"));
    ORIGIN.scope(origin.to_owned(), future).await
}
#[cfg(test)]
fn test_target(url: &url::Url) -> String {
    assert_eq!(url.host_str(), Some("assets.example.test"));
    ORIGIN
        .try_with(|origin| {
            format!(
                "{origin}{}{}",
                url.path(),
                url.query().map(|q| format!("?{q}")).unwrap_or_default()
            )
        })
        .expect("asset tests require an explicit non-forwarding loopback scope")
}
