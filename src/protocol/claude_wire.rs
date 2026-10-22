//! Claude wire allowlist. Software identity is not authentication or an OAuth grant.
pub const HEADERS: &[&str] = &[
    "accept",
    "accept-encoding",
    "user-agent",
    "x-app",
    "x-claude-code-session-id",
    "x-stainless-arch",
    "x-stainless-lang",
    "x-stainless-os",
    "x-stainless-package-version",
    "x-stainless-retry-count",
    "x-stainless-runtime",
    "x-stainless-runtime-version",
    "x-stainless-timeout",
    "anthropic-beta",
    "anthropic-version",
    "anthropic-dangerous-direct-browser-access",
];

pub fn valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && value.bytes().all(|b| (32..=126).contains(&b))
}

/// Only the observed Messages beta switch is public forwarding surface.
/// Arbitrary query credentials/controls must not ride the native evidence path.
pub fn query(value: &str) -> Result<&str, &'static str> {
    let pairs: Vec<_> = url::form_urlencoded::parse(value.as_bytes()).collect();
    if pairs.len() != 1
        || pairs[0].0 != "beta"
        || pairs[0].1.is_empty()
        || pairs[0].1.len() > 1024
        || !pairs[0]
            .1
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_,".contains(&b))
    {
        return Err("Claude native Messages only accepts one beta query parameter");
    }
    Ok(value)
}

pub fn identity(ua: &str, app: &str) -> bool {
    if app != "cli" || !valid(ua) {
        return false;
    }
    let Some(rest) = ua.strip_prefix("claude-cli/") else {
        return false;
    };
    let Some((version, mode)) = rest.split_once(' ') else {
        return false;
    };
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && matches!(mode, "(external, cli)" | "(external, sdk-cli)")
}

pub const USER_AGENT: &str = "claude-cli/2.1.278 (external, sdk-cli)";
/// HTTP/1 casing/order measured from this reference version, not a generic Node claim.
pub const HEADER_ORDER: &[&str] = &[
    "Accept",
    "Content-Type",
    "User-Agent",
    "X-Claude-Code-Session-Id",
    "X-Stainless-Arch",
    "X-Stainless-Lang",
    "X-Stainless-OS",
    "X-Stainless-Package-Version",
    "X-Stainless-Retry-Count",
    "X-Stainless-Runtime",
    "X-Stainless-Runtime-Version",
    "X-Stainless-Timeout",
    "anthropic-beta",
    "anthropic-dangerous-direct-browser-access",
    "anthropic-version",
    "x-api-key",
    "x-app",
    "Connection",
    "Host",
    "Accept-Encoding",
    "Content-Length",
];
pub fn platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "MacOS",
        "linux" => "Linux",
        _ => "Unknown",
    }
}
pub fn architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "x32",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_identity_is_bounded_not_a_wildcard() {
        assert!(identity(USER_AGENT, "cli"));
        assert!(identity("claude-cli/2.1.1 (external, cli)", "cli"));
        for ua in [
            "private-device",
            "claude-cli/2x (external, cli)",
            "claude-cli/2.1.1\r\nx: y",
        ] {
            assert!(!identity(ua, "cli"));
        }
        assert!(!identity(USER_AGENT, "browser"));
        assert!(!valid(&"x".repeat(4097)));
        for name in [
            "authorization",
            "x-api-key",
            "cookie",
            "proxy-authorization",
            "x-internal-api-key",
        ] {
            assert!(!HEADERS.contains(&name));
        }
    }
}
