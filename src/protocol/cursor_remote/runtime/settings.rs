//! Resolve local proxy policy at normal Gateway startup; zero external runtime dependencies.
use super::Settings;
use hyper_util::client::proxy::matcher::Matcher;
use std::ffi::OsStr;

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

// All origins used by the qualification and Run modules must use the same
// compatible tunnel. A bypass for even one origin must not become a direct call.
const ORIGINS: [&str; 3] = [
    "https://api2.cursor.sh/",
    "https://cursor.com/",
    "https://agentn.global.api5.cursor.sh/",
];

pub(super) fn resolve() -> Result<Settings, &'static str> {
    let explicit = std::env::var_os("GATEWAY_CURSOR_REMOTE_PROXY_PORT");
    let proxy_port = match explicit {
        // An invalid explicit override is an error, never permission to fall back.
        Some(value) => explicit_port(&value)?,
        None => proxy_port(
            &crate::http_client::native_proxy_matcher(),
            crate::http_client::native_proxy_https_source(),
        )?,
    };
    Ok(Settings {
        proxy_port,
        #[cfg(test)]
        worker: None,
    })
}

fn explicit_port(value: &OsStr) -> Result<u16, &'static str> {
    value
        .to_str()
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .ok_or("proxy_invalid")
}

fn proxy_port(matcher: &Matcher, source: Option<&str>) -> Result<u16, &'static str> {
    // Matcher normalizes away these URL components. Check the selected source,
    // not its lossy output, without changing the ordinary HTTP proxy contract.
    let url = url::Url::parse(source.ok_or("proxy_required")?).map_err(|_| "proxy_unsupported")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("proxy_unsupported");
    }
    let selected = url.port_or_known_default().ok_or("proxy_unsupported")?;
    if selected == 0 {
        return Err("proxy_unsupported");
    }
    for origin in ORIGINS {
        let proxy = matcher
            .intercept(&origin.parse().expect("fixed Cursor HTTPS origin"))
            .ok_or("proxy_required")?;
        let uri = proxy.uri();
        // The pure-Rust transport speaks unauthenticated HTTP CONNECT to IPv4 loopback.
        // Never discard credentials or silently reinterpret another proxy endpoint.
        if proxy.basic_auth().is_some()
            || uri.scheme_str() != Some("http")
            || uri.host() != Some("127.0.0.1")
            || uri.port_u16().unwrap_or(80) != selected
        {
            return Err("proxy_unsupported");
        }
    }
    Ok(selected)
}
