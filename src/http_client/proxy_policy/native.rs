//! Native Antigravity IO uses the same frozen proxy sources and explicit-empty bypass contract.
use super::{bypass_text, frozen_addresses, platform_bypass, Addresses};
use hyper_util::client::proxy::matcher::Matcher;

fn build(addresses: &Addresses, bypass: Option<String>) -> Matcher {
    let mut builder = Matcher::builder();
    if let Some(http) = &addresses.http {
        builder = builder.http(http);
    }
    if let Some(https) = &addresses.https {
        builder = builder.https(https);
    }
    builder.no(bypass.as_deref().unwrap_or("")).build()
}

pub(crate) fn matcher() -> Matcher {
    let addresses = frozen_addresses();
    #[cfg(target_os = "macos")]
    if addresses.is_empty() {
        return Matcher::from_system();
    }
    let environment = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .ok();
    let system = environment.is_none().then(platform_bypass).flatten();
    build(addresses, bypass_text(environment, system))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_preserves_split_sources_and_bypass() {
        let addresses = Addresses {
            http: Some("http://127.0.0.1:18080".into()),
            https: None,
        };
        let matcher = build(&addresses, Some("".into()));
        assert!(matcher
            .intercept(&"https://example.test/".parse().unwrap())
            .is_none());
        assert!(matcher
            .intercept(&"http://example.test/".parse().unwrap())
            .is_some());
        let bypass = bypass_text(None, Some("*.example.test;<local>".into()));
        assert!(build(&addresses, bypass)
            .intercept(&"http://sub.example.test/".parse().unwrap())
            .is_none());
    }
}
