//! Same-origin pinned transport. Only explicit monitoring auth; no proxies or redirects.
use futures::StreamExt;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub(super) fn checked_url(input: &str) -> anyhow::Result<url::Url> {
    let mut url = url::Url::parse(input).map_err(|_| anyhow::anyhow!("Invalid site URL"))?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && input.len() <= 2048,
        "Site URL must be HTTP(S), without credentials, query or fragment"
    );
    // Templates belong to the configured site origin, not an arbitrary credential API path.
    url.set_path("/");
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_only_by_default_and_explicit_private_never_permits_metadata() {
        for ip in ["127.0.0.1", "10.0.0.1", "192.168.1.1", "::1", "fd00::1"] {
            assert!(!allowed_ip(ip.parse().unwrap(), false), "{ip}");
            assert!(allowed_ip(ip.parse().unwrap(), true), "{ip}");
        }
        for ip in [
            "169.254.169.254",
            "100.100.100.200",
            "0.0.0.0",
            "224.0.0.1",
            "192.0.2.1",
            "fe80::1",
            "::ffff:169.254.169.254",
            "2002:7f00:1::1",
            "2001::1",
        ] {
            assert!(!allowed_ip(ip.parse().unwrap(), false), "{ip}");
            assert!(!allowed_ip(ip.parse().unwrap(), true), "{ip}");
        }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(allowed_ip(ip.parse().unwrap(), false), "{ip}");
        }
    }
}

pub(super) fn allowed_ip(ip: IpAddr, private: bool) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            if a == 0
                || a >= 224
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (18..=19).contains(&b))
                || (a == 192 && b == 0)
            {
                return false;
            }
            private || !(ip.is_private() || ip.is_loopback())
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return allowed_ip(IpAddr::V4(v4), private);
            }
            if ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unicast_link_local()
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0xdb8)
                || ip.segments()[0] == 0x2002
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0)
                || ip.segments()[0] & 0xffc0 == 0xfec0
            {
                return false;
            }
            if ip.is_loopback() || ip.is_unique_local() {
                return private;
            }
            ip.segments()[0] & 0xe000 == 0x2000
        }
    }
}

pub(super) async fn client(origin: &url::Url, private: bool) -> anyhow::Result<rquest::Client> {
    pinned_client(origin, private, Duration::from_secs(4)).await
}

/// Large public region HTML gets a separate deadline inside its finite 35-second discovery budget.
pub(super) async fn regional_client(origin: &url::Url) -> anyhow::Result<rquest::Client> {
    pinned_client(origin, false, Duration::from_secs(30)).await
}

async fn pinned_client(
    origin: &url::Url,
    private: bool,
    timeout: Duration,
) -> anyhow::Result<rquest::Client> {
    let host = origin
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Missing host"))?;
    let port = origin
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("Missing port"))?;
    let host = host.trim_matches(['[', ']']);
    let addresses: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::lookup_host((host, port)),
        )
        .await??
        .take(17)
        .collect()
    };
    anyhow::ensure!(
        !addresses.is_empty()
            && addresses.len() <= 16
            && addresses.iter().all(|a| allowed_ip(a.ip(), private)),
        "Site resolves to a prohibited network address"
    );
    Ok(rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .timeout(timeout)
        .resolve_to_addrs(host.to_owned(), addresses)
        .build()?)
}

pub(super) async fn json(
    client: &rquest::Client,
    url: &str,
    body: Option<&serde_json::Value>,
    auth: &super::MonitoringAuth,
) -> Result<serde_json::Value, (String, Option<u16>)> {
    let request = match body {
        Some(body) => client.post(url).json(body),
        None => client.get(url),
    };
    let request = match auth.kind {
        super::MonitoringAuthKind::None => request,
        kind => {
            let token = auth
                .token
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ("authentication_required".into(), None))?;
            let value = if kind == super::MonitoringAuthKind::Bearer {
                format!("Bearer {token}")
            } else {
                token.to_owned()
            };
            let mut value = rquest::header::HeaderValue::from_str(&value)
                .map_err(|_| ("invalid_monitoring_credential".into(), None))?;
            value.set_sensitive(true);
            let header = if kind == super::MonitoringAuthKind::Bearer {
                "authorization"
            } else {
                auth.header.as_deref().unwrap_or("x-api-key")
            };
            request.header(header, value)
        }
    };
    let (status, bytes) =
        bounded_response(request.header("Accept", "application/json"), 1_048_576).await?;
    serde_json::from_slice(&bytes).map_err(|_| ("invalid_json".into(), Some(status)))
}

/// Shared bound for official HTML bootstrap and public JSON; no unbounded response.text().
pub(super) async fn bounded_body(
    request: rquest::RequestBuilder,
) -> Result<Vec<u8>, (String, Option<u16>)> {
    bounded_response(request, 1_048_576)
        .await
        .map(|(_, bytes)| bytes)
}

/// Azure's published service/region matrix is about 7 MiB; only that fixed HTML reader uses this cap.
pub(super) async fn regional_html(
    request: rquest::RequestBuilder,
) -> Result<Vec<u8>, (String, Option<u16>)> {
    bounded_response(request, 8_388_608)
        .await
        .map(|(_, bytes)| bytes)
}

async fn bounded_response(
    request: rquest::RequestBuilder,
    max_bytes: usize,
) -> Result<(u16, Vec<u8>), (String, Option<u16>)> {
    let response = request.send().await.map_err(|e| {
        (
            if e.is_timeout() {
                "timeout"
            } else {
                "transport_error"
            }
            .into(),
            None,
        )
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err((
            match status {
                401 | 403 => "authentication_required",
                404 | 405 => "not_supported",
                429 => "rate_limited",
                _ => "http_error",
            }
            .into(),
            Some(status),
        ));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ("body_interrupted".into(), Some(status)))?;
        if chunk.len() > max_bytes - bytes.len() {
            return Err(("response_too_large".into(), Some(status)));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, bytes))
}
